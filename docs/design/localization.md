# Localization

How user-facing text is named, where it lives and how CI keeps it complete. The principles are in [architecture: localization & accessibility](../architecture.md#localization--accessibility).

## Where messages live

```text
/locales
  en/                    source locale; every other locale falls back to it
    rock-paper-scissors.ftl
    cli.ftl              (with the CLI, issue 6)
  de/                    same file names, same keys
    rock-paper-scissors.ftl
```

- One [Fluent](https://projectfluent.org/) file per crate or game, named after the game ID for a game (`mau-mau.ftl`) and after the crate name without the `libretable-` prefix for any other crate (`cli.ftl`, `game-core.ftl`). Every locale has the same files with the same keys.
- The files are shared by Rust, Swift and web. Rust embeds them at compile time through `libretable-l10n` (below); other clients will read the same files.
- Language-dependent game content (word lists) is game data, not a message.

## Message IDs

- Lowercase ASCII letters, digits and single inner hyphens, like a `GameId`. The conformance suite rejects anything else.
- IDs are global: one ID is defined once per locale across all files. A game prefixes its IDs with its game ID or a short prefix used by no other game and kept for good (rock-paper-scissors uses `rps-`). Mau-Mau would use `mau-mau-too-many-seats`, `mau-mau-wish-hearts`.
- **A game's display name is the message whose ID is its game ID** (`rock-paper-scissors = Rock, Paper, Scissors`). The game ID itself is never translated.
- Suggested suffix patterns inside a prefix: `-zone-…` for zone labels, `-indicator-…` for indicator labels, option choices named after what they choose (`rps-rock`), invalid-variant and illegal-move reasons named after the problem (`rps-too-few-seats`, `rps-not-pending`).
- Accessibility texts belong to their message as attributes (`.hint`, `.label`) rather than as separate IDs. Attributes are checked like messages.
- Terms (`-term`) are local helpers of a locale (German suit names, grammatical gender) and need no counterpart in other locales.

## Where IDs come from

The deterministic crates (`game-core`, `games/*`, `bots`, `conformance`) return message IDs and semantic values only and never depend on Fluent; `deny.toml` bans `fluent-bundle` outside `libretable-l10n` and `libretable-l10n` outside the client crates. A game emits IDs as:

- `InvalidVariant::reason`, `IllegalMove::reason`;
- `Zone::label`, `Indicator::label`, `IndicatorValue::Message`;
- `Choice::Option`;
- its `Game::ID` (display name).

Face keys (`queen-of-hearts`) are not message IDs yet; how renderers name components is decided with the text renderer and Mau-Mau.

### Discovering the IDs used in code

Each game declares every ID its code can emit in a `pub const MESSAGE_IDS: &[&str]`, its game ID included, and hands it to its conformance suite (`Suite::message_ids`). Two checks hold that list between code and catalog:

1. **Code → list:** the conformance suite fails (`Check::Messages`) when a random match emits an ID that is not in the list: a table label, a message indicator value, an option choice of a legal move or the reason of a rejected move. IDs the matches cannot reach (invalid-variant reasons) rely on the list alone and on the game's rule tests.
2. **List → catalog:** the localization check fails when a listed ID has no `en` message with a value.

A declared list was preferred over scanning the source for string literals: scanning cannot tell message IDs from other strings and misses computed IDs, while the conformance suite sees exactly what a game emits.

Client crates (the CLI, later server and bindings) declare their own IDs the same way and test them with `Localizer::missing` against the source localizer.

## The `libretable-l10n` crate

MPL-2.0, depends on `fluent-bundle`, `fluent-syntax` and `unic-langid` (MIT/Apache-2.0, checked by `cargo deny`).

```rust
use libretable_l10n::{Arg, Localizer};

let l10n = Localizer::new("de-CH");           // negotiates: de-CH → de
assert_eq!(l10n.locale(), "de");
l10n.format("rps-rock", &[]);                 // "Stein"
l10n.format("greeting", &[("name", Arg::from("Anna")), ("count", Arg::from(3))]);
l10n.try_format("no-such-id", &[]);           // None
l10n.format("no-such-id", &[]);               // "no-such-id"
```

- `Localizer::new(tag)` picks a supported locale for a BCP 47 tag: exact match, else the same language, else `en`. It never fails.
- `format(id, args)` looks the ID up in the chosen locale, then in `en`; an ID with no message in either comes back unchanged, so a gap is visible but never fatal. `try_format` returns `None` instead. A missing argument shows as `{$name}` in the text.
- Arguments are `Arg::Text` or `Arg::Number` (plural selection uses the locale's CLDR rules).
- No Unicode bidi isolation marks are inserted around arguments; revisit when a right-to-left locale arrives.
- `Localizer` is `Send + Sync`; build one per locale and share it.
- The catalog is embedded (`CATALOG`, `SOURCES`, `LOCALES` in `src/catalog.rs`); adding a file means adding one line per locale there. `Catalog` and `Source` also allow building a localizer from other sources, e.g. in tests.

## The CI check

`cargo test -p libretable-l10n` (its own `l10n` job in CI, also part of the workspace tests) fails when:

- a directory in `/locales` is not in `LOCALES`, or a file there is not embedded;
- a file does not parse;
- a key (message ID or `id.attribute`) is defined twice in a locale;
- a key of an `en` file is missing from that file in `de`, or `de` has a key or file that `en` lacks;
- an ID in a game's `MESSAGE_IDS` has no `en` message. The games checked are listed in `crates/l10n/tests/catalog.rs`; a new game adds its line there.

## Translations

German uses the informal *du*, as games among friends do, and gender-neutral wording where it reads naturally ("zwei Personen").
