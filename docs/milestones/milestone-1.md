# Milestone 1: walking skeleton

Goal: one card game (Mau-Mau) runs end to end through every core layer — rules, conformance suite, bots, CLI with text renderer, dev server and web multiclient. Each item below is one issue and, as a rule, one PR.

| # | Issue | Acceptance criteria |
| --- | --- | --- |
| 1 | Workspace, CI and licences | Cargo workspace with empty `game-core`; GitHub Actions running `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo deny check`; `deny.toml` allowing only licences compatible with the licence policy in `docs/policies.md`; LICENSE files for MPL-2.0, AGPL-3.0 and CC BY-SA 4.0 taken from the official sources; `.gitignore` |
| 2 | `game-core` trait and basic types | `Game` trait, `Pending`, `Seat`, `Observer`, `Event`, `Outcome`, `GameRng` (seeded, documented algorithm) and game record type per architecture; no I/O, no clock; unit tests for `GameRng` determinism |
| 3 | Rock-paper-scissors test game | Lives in `conformance`; exercises `All(seats)`; full rule tests |
| 4 | Conformance suite skeleton | Generic harness running N random games per game; checks determinism, legality, termination, round trips; green for rock-paper-scissors; leak test scaffolded |
| 5 | Selection protocol and table model v0 | Choice protocol and table model types; conformance check "every legal move reachable through choices, no choice path illegal" |
| 6 | CLI runner and text renderer | Play a game in the terminal (human or bot seats), replay a JSON record, simulate N games; text renderer shows a seat's table model and selectable items |
| 7 | Mau-Mau rules spec ([#24](https://github.com/Crazor/LibreTable/issues/24)) | `docs/games/mau-mau/rules.md`: standard rules plus typed house-rule options; signed off by the architect |
| 8 | Mau-Mau implementation ([#25](https://github.com/Crazor/LibreTable/issues/25)) | Crate `games/mau-mau`; passes conformance incl. leak test and opaque-ID reissue on reshuffle; rule tests from the spec; three golden replays |
| 9 | ISMCTS bot ([#26](https://github.com/Crazor/LibreTable/issues/26)) | Generic over `Game`; samples determinizations from the bot's view only; beats random play in Mau-Mau with statistical significance; playout benchmark |
| 10 | Dev server and web multiclient ([#7](https://github.com/Crazor/LibreTable/issues/7)) | Local WebSocket dev server running sessions; browser page with several seat views side by side, omniscient view (dev feature only), move log stepping; seats driven by hand or bot. Protocol: [design/server.md](../design/server.md); runs rock-paper-scissors until Mau-Mau exists |
| 11 | Localization scaffold ([#12](https://github.com/Crazor/LibreTable/issues/12)) | `/locales/en` and `/locales/de` Fluent files, one per crate or game; loader crate `l10n` outside the deterministic crates; CI fails on keys missing in `de` or IDs without an `en` message; lands before or with 6, which shows the first user-facing text |

After milestone 1: Rommé (needs the architect's house rules), Connect Four, then the web stage.
