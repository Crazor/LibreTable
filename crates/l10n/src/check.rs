//! The catalog check run in CI (`tests/catalog.rs`).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use fluent_syntax::ast::Entry;
use fluent_syntax::parser;

use crate::{Catalog, Source};

/// Something [`Catalog::check`] found wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// A file does not parse.
    Syntax {
        /// Locale of the file.
        locale: &'static str,
        /// The file.
        file: &'static str,
        /// One-based line of the error.
        line: usize,
        /// The parser's message.
        error: String,
    },
    /// A locale has no counterpart of a source-locale file.
    MissingFile {
        /// The locale lacking the file.
        locale: &'static str,
        /// The file.
        file: &'static str,
    },
    /// A file has no counterpart in the source locale, or belongs to a
    /// locale the catalog does not list.
    ExtraFile {
        /// Locale of the file.
        locale: &'static str,
        /// The file.
        file: &'static str,
    },
    /// A key is defined twice in one locale.
    Duplicate {
        /// The locale.
        locale: &'static str,
        /// The key: a message ID or `id.attribute`.
        key: String,
        /// The file of the second definition.
        file: &'static str,
    },
    /// A key of a source-locale file is not translated in that file of
    /// another locale.
    Untranslated {
        /// The locale lacking the key.
        locale: &'static str,
        /// The file.
        file: &'static str,
        /// The key: a message ID or `id.attribute`.
        key: String,
    },
    /// A key of a translation has no counterpart in the source-locale file.
    Orphaned {
        /// The locale defining the key.
        locale: &'static str,
        /// The file.
        file: &'static str,
        /// The key: a message ID or `id.attribute`.
        key: String,
    },
    /// An ID declared by code has no source-locale message with a value.
    NoSourceMessage {
        /// Where the ID is declared, e.g. `"rock-paper-scissors"`.
        origin: &'static str,
        /// The ID.
        id: &'static str,
    },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax {
                locale,
                file,
                line,
                error,
            } => write!(f, "locales/{locale}/{file}.ftl:{line}: {error}"),
            Self::MissingFile { locale, file } => {
                write!(f, "locales/{locale}/{file}.ftl is missing")
            }
            Self::ExtraFile { locale, file } => write!(
                f,
                "locales/{locale}/{file}.ftl has no source-locale counterpart \
                 or its locale is not supported"
            ),
            Self::Duplicate { locale, key, file } => {
                write!(f, "locales/{locale}/{file}.ftl: {key} is defined twice")
            }
            Self::Untranslated { locale, file, key } => {
                write!(f, "locales/{locale}/{file}.ftl: {key} is not translated")
            }
            Self::Orphaned { locale, file, key } => write!(
                f,
                "locales/{locale}/{file}.ftl: {key} is not in the source locale"
            ),
            Self::NoSourceMessage { origin, id } => {
                write!(f, "{origin}: message ID {id} has no source-locale message")
            }
        }
    }
}

/// Keys of one file: message IDs and `id.attribute`, in order of definition.
/// Terms (`-term`) are locale-specific helpers and not compared.
struct Keys {
    all: Vec<String>,
    /// Messages that have a value (not only attributes).
    with_value: BTreeSet<String>,
}

fn keys(source: &Source, problems: &mut Vec<Problem>) -> Keys {
    let resource = parser::parse(source.text).unwrap_or_else(|(partial, errors)| {
        for error in errors {
            problems.push(Problem::Syntax {
                locale: source.locale,
                file: source.file,
                line: source.text[..error.pos.start.min(source.text.len())]
                    .matches('\n')
                    .count()
                    + 1,
                error: error.kind.to_string(),
            });
        }
        partial
    });
    let mut keys = Keys {
        all: Vec::new(),
        with_value: BTreeSet::new(),
    };
    for entry in resource.body {
        if let Entry::Message(message) = entry {
            let id = message.id.name;
            keys.all.push(id.to_owned());
            if message.value.is_some() {
                keys.with_value.insert(id.to_owned());
            }
            for attribute in message.attributes {
                keys.all.push(format!("{id}.{}", attribute.id.name));
            }
        }
    }
    keys
}

impl Catalog {
    /// Checks the catalog and the message IDs declared by code; empty when
    /// all is well. Finds:
    ///
    /// - files that do not parse;
    /// - files present in the source locale but not in another locale, and
    ///   the reverse;
    /// - keys (message IDs and their attributes) defined twice in a locale;
    /// - keys of a source-locale file missing from that file in another
    ///   locale, and the reverse;
    /// - IDs in `code_ids` without a source-locale message. Each entry is
    ///   `(origin, ids)`, e.g. a game's `MESSAGE_IDS` under its game ID.
    #[must_use]
    pub fn check(&self, code_ids: &[(&'static str, &'static [&'static str])]) -> Vec<Problem> {
        let mut problems = Vec::new();
        // locale -> file -> keys
        let mut files: BTreeMap<&str, BTreeMap<&str, Keys>> = BTreeMap::new();
        for source in self.sources {
            let keys = keys(source, &mut problems);
            if !self.locales.contains(&source.locale) {
                problems.push(Problem::ExtraFile {
                    locale: source.locale,
                    file: source.file,
                });
                continue;
            }
            files
                .entry(source.locale)
                .or_default()
                .insert(source.file, keys);
        }

        for locale in self.locales {
            let mut seen = BTreeSet::new();
            for (file, keys) in files.get(locale).into_iter().flatten() {
                for key in &keys.all {
                    if !seen.insert(key.as_str()) {
                        problems.push(Problem::Duplicate {
                            locale,
                            key: key.clone(),
                            file,
                        });
                    }
                }
            }
        }

        let source_locale = self.source_locale();
        let empty = BTreeMap::new();
        let source_files = files.get(source_locale).unwrap_or(&empty);
        for locale in &self.locales[1..] {
            let translated = files.get(locale).unwrap_or(&empty);
            for (file, source_keys) in source_files {
                let Some(keys) = translated.get(file) else {
                    problems.push(Problem::MissingFile { locale, file });
                    continue;
                };
                let have: BTreeSet<&String> = keys.all.iter().collect();
                let want: BTreeSet<&String> = source_keys.all.iter().collect();
                for key in want.difference(&have) {
                    problems.push(Problem::Untranslated {
                        locale,
                        file,
                        key: (*key).clone(),
                    });
                }
                for key in have.difference(&want) {
                    problems.push(Problem::Orphaned {
                        locale,
                        file,
                        key: (*key).clone(),
                    });
                }
            }
            for file in translated.keys() {
                if !source_files.contains_key(file) {
                    problems.push(Problem::ExtraFile { locale, file });
                }
            }
        }

        for (origin, ids) in code_ids {
            for id in *ids {
                if !source_files
                    .values()
                    .any(|keys| keys.with_value.contains(*id))
                {
                    problems.push(Problem::NoSourceMessage { origin, id });
                }
            }
        }
        problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog(sources: &'static [Source]) -> Catalog {
        Catalog {
            locales: &["en", "de"],
            sources,
        }
    }

    // A macro, not a function, so the slices below are promoted to statics.
    macro_rules! source {
        ($locale:literal, $file:literal, $text:literal) => {
            Source {
                locale: $locale,
                file: $file,
                text: $text,
            }
        };
    }

    #[test]
    fn a_complete_catalog_passes() {
        let catalog = catalog(&[
            source!("en", "a", "x = X\n    .hint = H\n-term = T\n"),
            source!("de", "a", "x = Ix\n    .hint = Ha\n-other-term = U\n"),
        ]);
        assert_eq!(catalog.check(&[("a", &["x"])]), []);
    }

    #[test]
    fn finds_untranslated_and_orphaned_keys() {
        let catalog = catalog(&[
            source!("en", "a", "x = X\ny = Y\n    .hint = H\n"),
            source!("de", "a", "x = Ix\nz = Zett\n"),
        ]);
        assert_eq!(
            catalog.check(&[]),
            [
                Problem::Untranslated {
                    locale: "de",
                    file: "a",
                    key: "y".into()
                },
                Problem::Untranslated {
                    locale: "de",
                    file: "a",
                    key: "y.hint".into()
                },
                Problem::Orphaned {
                    locale: "de",
                    file: "a",
                    key: "z".into()
                },
            ]
        );
    }

    #[test]
    fn finds_missing_and_extra_files() {
        let catalog = catalog(&[
            source!("en", "a", "x = X\n"),
            source!("de", "b", "y = Y\n"),
            source!("fr", "a", "x = X\n"),
        ]);
        assert_eq!(
            catalog.check(&[]),
            [
                Problem::ExtraFile {
                    locale: "fr",
                    file: "a"
                },
                Problem::MissingFile {
                    locale: "de",
                    file: "a"
                },
                Problem::ExtraFile {
                    locale: "de",
                    file: "b"
                },
            ]
        );
    }

    #[test]
    fn finds_duplicates_across_files() {
        let catalog = catalog(&[
            source!("en", "a", "x = X\n"),
            source!("en", "b", "x = X again\n"),
            source!("de", "a", "x = Ix\n"),
            source!("de", "b", "x = Ix\n"),
        ]);
        let problems = catalog.check(&[]);
        assert_eq!(
            problems,
            [
                Problem::Duplicate {
                    locale: "en",
                    key: "x".into(),
                    file: "b"
                },
                Problem::Duplicate {
                    locale: "de",
                    key: "x".into(),
                    file: "b"
                },
            ]
        );
    }

    #[test]
    fn finds_code_ids_without_a_source_message() {
        let catalog = catalog(&[
            source!("en", "a", "x = X\nonly-attributes =\n    .hint = H\n"),
            source!(
                "de",
                "a",
                "x = Ix\nonly-attributes =\n    .hint = Ha\nde-only = D\n"
            ),
        ]);
        let problems = catalog.check(&[("game", &["x", "only-attributes", "de-only", "gone"])]);
        assert_eq!(
            problems[1..],
            [
                Problem::NoSourceMessage {
                    origin: "game",
                    id: "only-attributes"
                },
                Problem::NoSourceMessage {
                    origin: "game",
                    id: "de-only"
                },
                Problem::NoSourceMessage {
                    origin: "game",
                    id: "gone"
                },
            ]
        );
    }

    #[test]
    fn finds_syntax_errors_with_their_line() {
        let catalog = catalog(&[
            source!("en", "a", "x = X\n\nbroken\n"),
            source!("de", "a", "x = Ix\n"),
        ]);
        let problems = catalog.check(&[]);
        assert!(
            matches!(
                &problems[..],
                [Problem::Syntax {
                    locale: "en",
                    file: "a",
                    line: 3,
                    ..
                }]
            ),
            "{problems:?}"
        );
    }
}
