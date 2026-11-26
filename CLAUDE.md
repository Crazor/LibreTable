# CLAUDE.md

LibreTable (working name) is an open-source platform for turn-based card, dice and board games: a deterministic Rust core with generic ISMCTS bots, a server, a web client and native SwiftUI apps. The human owner acts as architect and writes no code; Claude implements.

## Read first

- [docs/architecture.md](docs/architecture.md) — principles, layers, core concepts. Binding.
- [docs/vision.md](docs/vision.md) — goals, roadmap, game selection.
- [docs/adr/](docs/adr/) — decisions; check before changing an interface.
- [docs/milestones/](docs/milestones/) — current milestone and its acceptance criteria.
- [docs/games/<game>/rules.md](docs/games/) — rules spec before touching a game.
- [docs/reference-games.md](docs/reference-games.md) and [docs/policies.md](docs/policies.md) when relevant.

## Hard rules

- Never add I/O, time, threads or global randomness to `game-core`, `games/*` or `bots`.
- All randomness goes through `GameRng`; all user-facing text goes through localization keys (Fluent, `/locales`).
- Bots and clients consume views only; `Observer::Omniscient` is gated behind the `dev` feature and never sent by a non-dev server.
- A new game is not done until it passes `conformance` and ships `rules.md` plus at least three golden replays.
- Changing a public interface, the wire format or a guiding principle requires an ADR in the same change.
- Do not copy code from GPL-licensed projects; check new dependencies against ADR-0002 (`cargo deny check`).
- Implement only public-domain games or licensed ones; commercial games are references only (see docs/policies.md).
- Code, comments, docs, commit messages, PR and issue texts are in English.
- The working name appears only in the crate prefix, the bundle ID and one display-name constant.

## Working style

- Prefer small changes with one concern each; keep docs in sync with the code in the same change.
- Before finishing, `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings` must pass (a Stop hook checks this).
- Prove behaviour with tests, the conformance suite, the CLI and the text renderer rather than by inspection.
- When a requirement is unclear or a rule is ambiguous, ask the architect instead of guessing.
