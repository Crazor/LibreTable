# cli

The `libretable-cli` crate builds the binary `table`: play a match in the terminal with human or bot seats, replay a JSON game record, simulate bot-only matches. Its `render` module is the text renderer. Matches run through [`Session`](session.md); human input goes through the selection protocol, so no illegal move can be entered.

## Usage

```text
cargo run -p libretable-cli -- <command>     # or target/debug/table <command>

table games
table play <game> [--seats human,bot,ismcts,...] [--seed <n>] [--variant <json>] [--record <file>] [--iterations <n>]
table replay <record.json> [--seat <n>] [--each]
table simulate <game> [-n <matches>] [--seats <count> | --bots bot,ismcts,...] [--seed <n>] [--variant <json>] [--max-moves <n>] [--iterations <n>]
table help
```

- `--lang <tag>` may be given anywhere and picks the locale; without it the CLI uses `LC_ALL`, `LC_MESSAGES` or `LANG` (`de_DE.UTF-8` → `de-DE`), else English. Unsupported locales fall back to English.
- Games are picked by ID from `libretable-registry` (shared with the server; `src/registry.rs` erases the game type for the commands); adding a game is one line there. Currently `mau-mau` and `rock-paper-scissors`.
- `--variant` is the game's config as JSON (e.g. `'{"rounds":1}'`); default is the standard ruleset.
- Seats are numbered from 0, as in records.
- **Seeds.** `--seed <n>` seeds a master `GameRng`; the match seed and one seed per bot derive from it (`session::derive_seed`), so a whole run repeats exactly. `play` without `--seed` takes one from the clock and prints it; `simulate` defaults to 0.
- **Seat kinds:** `human`, `bot` (the random bot, also `random`) and `ismcts` (the search bot, for games the registry marks `searchable`; otherwise an error). `--iterations` sets ISMCTS iterations per decision (default 500). See [bots](bots.md).
- **play.** Default seats `human,bot`. At a human seat the CLI shows that seat's table and the numbered selectable choices; input is a number, `u` (undo) or `q` (quit, also end of input). With more than one human seat a hand-over prompt (privacy screen) comes before each change of human seat and clears an interactive terminal. Bot moves are announced without revealing them. Without humans, the spectator table is shown whenever it changes. At the end: spectator table and result. `--record` writes the record, also of an abandoned match.
- **replay.** The game comes from the record's `game` field. The whole record is checked first; then every move is listed as its choice path (as seen by the moving seat), followed by the table of `--seat` (default spectator) and the result. `--each` shows the table after every move.
- **simulate.** Plays `-n` matches (default 100) with `--seats` random bots (default 2), or with the bots listed in `--bots`, and prints moves in total, wins (rank 1, ties included) and average score per seat. A match longer than `--max-moves` (default 100 000) is an error.
- Exit codes: 0 success, 1 runtime error, 2 wrong command line (usage is printed).
- The build ID in records is the compile-time `BUILD_ID` environment variable, else `cli-<version>`.

## Text renderer

`render::table(&TableModel, Observer)`, `render::prompt(&TableModel, seat, made, selectable)`, `render::choices`, `render::choice` and `render::outcome` return plain, deterministic text, so tests compare it with expected strings. Example (rock-paper-scissors, seat 0 after throwing):

```text
--- Table: seat 0 ---
Round: 1
Score (seat 0): 0
Score (seat 1): 0
[0] Throw: seat 0, player board, owner only, stack
    #0 token: Rock (shape: rock)
[1] Throw: seat 1, player board, owner only, stack
    (empty)
[64] Last round: shared, tableau, public, row
    (empty)
```

- Indicators first, then zones in the game's order: `[id] label: owner, kind, visibility, layout`, then one component per line: `#id kind: key (attributes)`, or `face down`, plus rotation if not 0.
- Choices: options by their message, components as the component plus its zone, zones as `[id] label`, numbers as is, subsets as their members.
- Face-up components are named by their face key, which is a message ID (`card-queen-of-hearts` → "Queen of Hearts", see [localization](localization.md)), followed by their attributes.

## Localization

All text goes through `l10n::tr` / `tr!`, which formats with one process-wide `libretable_l10n::Localizer` chosen at start-up. The CLI's own messages are in `locales/*/cli.ftl`, their IDs listed in `l10n::MESSAGE_IDS` and tested with `Localizer::missing`. Game texts (zone and indicator labels, option choices, reasons, the game's display name = the message whose ID is the game ID) go through the same localizer. Average scores are formatted in code with a decimal point, not per locale.

Tests: `crates/cli/tests/render.rs` (snapshots), `crates/cli/tests/cli.rs` (the binary end to end: games, simulate, play with bots and with piped human input, replay, errors, locale choice).
