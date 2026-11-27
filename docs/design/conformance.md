# conformance

The `libretable-conformance` crate holds the generic conformance suite described in [architecture: testing & conformance suite](../architecture.md#testing--conformance-suite). It is a development tool and never shipped.

## Harness

`conformance::run(&Suite<G>)` plays `matches` random matches for every `(config, seats)` setup of a game and returns a `Report` or the first `Failure`. A `Suite` is a plain struct, not a trait, so a game crate can describe itself without running into the orphan rule. Besides the setups it holds:

- `seed`: match seeds and the random move choices derive from it, so a run is reproducible. The driver's choices use their own `GameRng`, never the match's.
- `step_bound(config, seats)`: the most moves plus chance resolutions a match may take.
- `candidate_moves(state)`: moves to try besides the legal ones, so that rejection is tested. Ideally every well-formed move that could be legal somewhere in the game.
- `hidden_variants(state, seat, rng)`: for the leak test, a few states that differ from `state` only in what is hidden from `seat`; `None` for games without hidden information.
- `message_ids`: every Fluent message ID the game can emit, its game ID included (the game's `MESSAGE_IDS`, see [localization.md](localization.md)).

Checks at every decision point (also before each chance step) and at the end:

| Check | What it verifies |
| --- | --- |
| Legality | `pending` is `Finished` exactly when `outcome` is `Some`; `All`/`Any` sets are non-empty and inside the match; exactly the pending seats have legal moves, without duplicates; every legal move applies; every other candidate move, every move of a non-pending seat and of a seat outside the match is rejected and leaves state and generator unchanged; the outcome has one result per seat; nothing is legal after the end. |
| Termination | The match finishes within `step_bound`. |
| Round trips | Config, state, every seat's and the spectator's view, legal moves, events, outcome and record survive JSON unchanged, and re-serializing gives identical text. |
| Determinism | Replaying the record, before and after its JSON round trip, reaches the identical state and generator. |
| Leak (scaffold) | A seat's view is identical in every declared hidden variant. |
| Table | Every seat's and the spectator's table model passes `check_table`: unique zone and component IDs, no face shown in a hidden zone or in another seat's owner-only zone. |
| Selection | `check_selection` for every seat: choice paths are unique and prefix-free, name only things on the seat's table, reach every legal move and each reached move applies. |
| Messages | Before any match: `message_ids` is well-formed (kebab-case), duplicate-free and holds the game ID. During matches: every zone and indicator label, message indicator value, option choice of a legal move and reason of a rejected move is in `message_ids`. |

A `Failure` names the check, explains it and carries the partial record as JSON (empty for a malformed `message_ids`, found before any match), so the failing match replays in the CLI (`table replay`, see [cli](cli.md)).

Not checked yet: the binary wire encoding (once ADR-0003 is accepted), opaque-ID reissue on hidden shuffles (with Mau-Mau, issue 8), game-declared invariants, bot-driven matches and cross-target determinism (native vs. Wasm). The omniscient view is not checked because the suite does not depend on the `dev` feature.

Tests: `crates/conformance/tests/suite.rs` runs rock-paper-scissors (2, 3 and 7 seats × 100 matches) and shows each check catching a deliberately broken game.

## Rock-paper-scissors test game

`conformance::rock_paper_scissors` exercises simultaneous decisions (`Pending::All`), which no shipped game in milestone 1 needs. The rules are in the module documentation and were confirmed by the architect on 2026-11-26; in short:

- 2 to 64 seats; variant `rounds` (at least 1, default 3).
- Each round every pending seat throws once, in any arrival order. The game keeps the throws sealed in its state, removes each seat that threw from the pending set, and resolves when the last seat has thrown.
- Exactly two different shapes: every seat with the winning shape wins the round. One shape or all three: draw. Draws count as rounds.
- Score = rounds won; rank by score, ties share. A match takes exactly `seats × rounds` moves, no chance step.
- A seat's view shows only its own sealed throw; spectators see who has thrown, not what. The omniscient view (`dev` feature) shows all.
- Table: one owner-only player board per seat with its sealed throw (face down for others), a public tableau with the last round's throws, score and round indicators. A throw is one option choice (`rps-rock`, `rps-paper`, `rps-scissors`).

- Messages: `MESSAGE_IDS` lists every ID the game emits (prefix `rps-`, display name `rock-paper-scissors`); the texts are in `locales/*/rock-paper-scissors.ftl`.

Rule tests are in `crates/conformance/tests/rock_paper_scissors.rs`.
