//! The one place user-facing text comes from.
//!
//! Every string the CLI prints goes through [`tr`] (or the [`tr!`](crate::tr)
//! macro) with a Fluent message ID: the CLI's own (`cli-…`, listed in
//! [`MESSAGE_IDS`], messages in `locales/*/cli.ftl`) and the IDs games put
//! into table models, choices and errors. Formatting uses one process-wide
//! [`Localizer`], chosen once by [`init`]; without `init` it is English.

use std::sync::OnceLock;

use libretable_l10n::{Arg, Localizer};

/// Every message ID the CLI itself uses. Tested against the `en` catalog.
pub const MESSAGE_IDS: &[&str] = &[
    "cli-usage",
    "cli-games-header",
    "cli-games-entry",
    "cli-error",
    "cli-error-unknown-command",
    "cli-error-missing-argument",
    "cli-error-unknown-option",
    "cli-error-option-value",
    "cli-error-bad-number",
    "cli-error-unknown-game",
    "cli-error-seat-kind",
    "cli-error-seat-range",
    "cli-error-seat-count",
    "cli-error-variant-json",
    "cli-error-invalid-variant",
    "cli-error-read",
    "cli-error-write",
    "cli-error-io",
    "cli-error-record-json",
    "cli-error-replay-schema",
    "cli-error-replay-wrong-game",
    "cli-error-replay-not-pending",
    "cli-error-replay-illegal",
    "cli-error-not-pending",
    "cli-error-not-selectable",
    "cli-error-incomplete",
    "cli-error-ambiguous",
    "cli-error-illegal",
    "cli-error-too-long",
    "cli-play-header",
    "cli-seat-human",
    "cli-seat-bot",
    "cli-prompt",
    "cli-prompt-item",
    "cli-prompt-made",
    "cli-prompt-input",
    "cli-input-invalid",
    "cli-handover",
    "cli-bot-acted",
    "cli-quit",
    "cli-record-written",
    "cli-table-header",
    "cli-observer-seat",
    "cli-observer-spectator",
    "cli-indicator",
    "cli-indicator-owned",
    "cli-zone",
    "cli-zone-empty",
    "cli-owner-seat",
    "cli-owner-shared",
    "cli-zone-kind-draw-pile",
    "cli-zone-kind-hand",
    "cli-zone-kind-discard",
    "cli-zone-kind-trick",
    "cli-zone-kind-tableau",
    "cli-zone-kind-player-board",
    "cli-zone-kind-score-sheet",
    "cli-zone-kind-board",
    "cli-zone-kind-other",
    "cli-visibility-public",
    "cli-visibility-owner",
    "cli-visibility-hidden",
    "cli-layout-stack",
    "cli-layout-fan",
    "cli-layout-row",
    "cli-layout-grid",
    "cli-layout-free",
    "cli-component-kind-card",
    "cli-component-kind-tile",
    "cli-component-kind-piece",
    "cli-component-kind-token",
    "cli-component-kind-die",
    "cli-component-kind-marker",
    "cli-component-hidden",
    "cli-component-shown",
    "cli-component-shown-attributes",
    "cli-attribute",
    "cli-rotated",
    "cli-choice-component",
    "cli-choice-zone",
    "cli-choice-subset",
    "cli-choice-none",
    "cli-outcome-header",
    "cli-outcome-seat",
    "cli-outcome-team",
    "cli-unfinished",
    "cli-replay-header",
    "cli-replay-move",
    "cli-simulate-summary",
    "cli-simulate-seat",
];

static LOCALIZER: OnceLock<Localizer> = OnceLock::new();

/// Chooses the locale for the rest of the process: `lang` (from `--lang`)
/// if given, else the POSIX locale variables `LC_ALL`, `LC_MESSAGES`, `LANG`
/// in that order, else English. Has no effect after the first message was
/// formatted.
pub fn init(lang: Option<&str>) {
    let tag = lang.map(str::to_owned).or_else(libretable_l10n::env_locale);
    let _ = LOCALIZER.set(Localizer::new(tag.as_deref().unwrap_or("en")));
}

/// The chosen locale, e.g. `"de"`.
#[must_use]
pub fn locale() -> &'static str {
    localizer().locale()
}

fn localizer() -> &'static Localizer {
    LOCALIZER.get_or_init(|| Localizer::new("en"))
}

pub use libretable_l10n::posix_to_bcp47;

/// A message argument owned by the caller of [`tr`]: numbers stay numbers,
/// so Fluent can pick plural forms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    /// Text, inserted as is.
    Text(String),
    /// A number.
    Number(i64),
}

/// Converts message arguments for [`tr!`](crate::tr).
pub trait IntoValue {
    /// The argument.
    fn into_value(self) -> Value;
}

macro_rules! number_value {
    ($($t:ty),*) => {$(
        impl IntoValue for $t {
            fn into_value(self) -> Value {
                i64::try_from(self).map_or_else(|_| Value::Text(self.to_string()), Value::Number)
            }
        }
    )*};
}
number_value!(i8, i16, i32, i64, u8, u16, u32, u64, usize);

impl IntoValue for String {
    fn into_value(self) -> Value {
        Value::Text(self)
    }
}

impl IntoValue for &String {
    fn into_value(self) -> Value {
        Value::Text(self.clone())
    }
}

impl IntoValue for &str {
    fn into_value(self) -> Value {
        Value::Text(self.to_owned())
    }
}

/// Formats message `id` with named `args` in the chosen locale. An ID
/// without any message comes back unchanged (visible, never fatal).
#[must_use]
pub fn tr(id: &str, args: &[(&str, Value)]) -> String {
    let args: Vec<(&str, Arg<'_>)> = args
        .iter()
        .map(|(name, value)| {
            let arg = match value {
                Value::Text(text) => Arg::Text(text),
                Value::Number(n) => Arg::Number(*n),
            };
            (*name, arg)
        })
        .collect();
    localizer().format(id, &args)
}

/// `tr!("id", name = value, ...)`: [`tr`] with arguments converted by
/// [`IntoValue`] (integers as numbers, strings as text).
#[macro_export]
macro_rules! tr {
    ($id:expr) => {
        $crate::l10n::tr($id, &[])
    };
    ($id:expr, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::l10n::tr(
            $id,
            &[$((stringify!($name), $crate::l10n::IntoValue::into_value($value))),+],
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cli_message_exists_in_english() {
        assert_eq!(
            Localizer::new("en").missing(MESSAGE_IDS),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn every_cli_message_is_listed() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../locales/en/cli.ftl"
        ));
        for line in source.lines() {
            if let Some((id, _)) = line.split_once(" =") {
                if id.starts_with("cli-") {
                    assert!(MESSAGE_IDS.contains(&id), "{id} not in MESSAGE_IDS");
                }
            }
        }
    }

    #[test]
    fn ids_are_unique() {
        for (index, id) in MESSAGE_IDS.iter().enumerate() {
            assert!(!MESSAGE_IDS[index + 1..].contains(id), "{id} twice");
        }
    }

    #[test]
    fn formats_with_numbers_and_text() {
        assert_eq!(tr!("cli-observer-seat", seat = 2_u8), "seat 2");
        assert_eq!(tr!("rps-rock"), "Rock");
        assert_eq!(tr!("no-such-message"), "no-such-message");
        assert_eq!(
            tr!("cli-layout-grid", columns = 1_u16),
            "grid of one column"
        );
    }

    #[test]
    fn posix_locales_map_to_tags() {
        assert_eq!(posix_to_bcp47("de_DE.UTF-8"), "de-DE");
        assert_eq!(posix_to_bcp47("de_AT@euro"), "de-AT");
        assert_eq!(posix_to_bcp47("C"), "en");
        assert_eq!(posix_to_bcp47("POSIX"), "en");
    }
}
