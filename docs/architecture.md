# Architecture

This document describes the stable architecture: principles, layers and core concepts. Details of individual components, the decisions behind them and their open points live in [design/](design/).

## Guiding principles

These principles are binding.

1. **Deterministic core.** Given a seed and a move list, a game always reaches the same state. No clock, no I/O, no global randomness, no threads inside game logic.
2. **Views, not state.** Nothing outside the core ever sees the full state, except the omniscient debug view in local development builds. Bots, clients and transports receive views.
3. **Moves as data.** A move is a serializable value. Where it comes from (touch, bot, Game Center, server, replay) is irrelevant to the core.
4. **Bots are just players.** A bot receives the same view a human would and returns a move. No privileged access, ever.
5. **The interface is the product.** Games, bots, renderers and transports are plug-ins against stable interfaces.
6. **Machine-checkable correctness.** Every game passes the conformance suite; every rule fix comes with a replay or test.
7. **Thin platform layers.** Platform code renders and forwards input. Logic that can live in the core lives in the core.
8. **English everywhere.** Code, identifiers, tools, docs, commits, PRs and issues are English. All user-facing text is localizable from day one; German is the first and best-maintained translation.
9. **Privacy by default.** Store as little personal data as possible; no tracking, no ads.

## System overview

All logic lives in Rust crates that every client and the server link; platform code only renders and forwards input. Each layer depends only on the layers below it.

```text
┌───────────────────────────────────────────────────────────────────────┐
│ Clients     CLI runner │ Web (dev multiclient, app) │ SwiftUI apps │ Android (later) │
├───────────────────────────────────────────────────────────────────────┤
│ Bindings    native Rust │ Wasm (wasm-bindgen) │ UniFFI (Swift, Kotlin)  │
├──────────────────────────────────────────────┬────────────────────────┤
│ session     seats, match flow, bot takeover, │ server (axum)          │
│             transports: local, Game Center,  │◄─ WebSocket ─► clients │
│             WebSocket                        │ links core/games/bots  │
├───────────────┬────────────────┬─────────────┴────────────────────────┤
│ games/*       │ bots           │ conformance                          │
│ Mau-Mau, …    │ ISMCTS, heur.  │ suite run for every game             │
├───────────────┴────────────────┴──────────────────────────────────────┤
│ game-core   Game trait, views, table model, selection protocol,       │
│             GameRng, records (no I/O, no clock)                       │
└───────────────────────────────────────────────────────────────────────┘
```

The session runs a match locally or forwards it through a transport; the server runs the same session code authoritatively for online matches.

## Game core: the `Game` trait

Every game is a pure state machine behind one trait. The sketch fixes the shape; exact names are in [design/game-core.md](design/game-core.md).

```rust
pub trait Game {
    type Config: Variant;          // validated variant / house-rule set
    type State: Clone;             // full, authoritative state
    type Move: Clone + Eq + Serialize + Deserialize;
    type View: Serialize;          // what one observer may know
    type Event: Serialize;         // what happened (per game)

    const ID: GameId;              // e.g. "mau-mau"

    fn setup(config: &Self::Config, seats: SeatCount, rng: &mut GameRng) -> Self::State;

    /// Who must decide now.
    fn pending(state: &Self::State) -> Pending;          // All(seats) | Any(seats) | Chance | Finished

    fn legal_moves(state: &Self::State, seat: Seat) -> Vec<Self::Move>;

    /// Applies one move. Chance (shuffle, draw, dice) uses the rng only.
    fn apply(state: &mut Self::State, seat: Seat, mv: &Self::Move,
             rng: &mut GameRng) -> Result<Vec<Self::Event>, IllegalMove>;

    /// Resolves a chance step while `pending` is `Chance`.
    fn apply_chance(state: &mut Self::State, rng: &mut GameRng) -> Vec<Self::Event>;

    fn view(state: &Self::State, observer: Observer) -> Self::View;
    fn table(view: &Self::View) -> TableModel;           // generic rendering model
    fn choices(view: &Self::View, mv: &Self::Move) -> Vec<Choice>; // selection path

    fn outcome(state: &Self::State) -> Option<Outcome>;  // scores, ranks, per-seat/per-team result
}
```

- **Simultaneous decisions and real-time play** are first-class. `pending` returns either `All(seats)`, where every listed seat submits once and the moves resolve together (sealed bids, card passing, rock-paper-scissors), or `Any(seats)`, where any listed seat may act at any time, moves apply in arrival order and a move invalidated by a faster one is rejected (Ligretto/Nerts). Time stays outside the core: the arrival order chosen by the authoritative session is part of the move log, so real-time games remain deterministic and replayable.
- **Out-of-turn decisions** (a defender's dice, merger decisions, trade answers, Kontra) are just changes of `pending` inside a turn. Optional reactions get a default move; timeouts live in the session, not the core.
- **Chance** is explicit. All randomness flows through `GameRng`, a seeded, versioned PRNG (ChaCha12, see [design/game-core.md](design/game-core.md)) owned by the session, so a game is fully defined by config, seed and move list.
- **Events** describe what happened (card moved, revealed, shuffled, trick won). Renderers animate from them; logs and replays read them.
- **Teams and partnerships** (Canasta, Doppelkopf, Bridge, Skat's declarer vs. defenders) are part of `Outcome` and of the view rules from the start; partners still see only their own hands.
- **Large move spaces.** `legal_moves` cannot always be a full list (numeric choices, any free node, rearranging all melds in tile rummy). The core also offers lazy enumeration and validation of a submitted draft (open point in [design/game-core.md](design/game-core.md#open-points)).
- **History in the state** where rules need it (repetition draws, last-round triggers).
- **No dependencies** beyond `core`/`alloc` plus `serde` in the core crate, so it compiles to native, Wasm and mobile targets unchanged.

## Views & hidden information

Hidden information is enforced by construction: an observer can only ever receive a `View`, and a view contains exactly what that observer may know.

| Observer | Sees | Available in |
| --- | --- | --- |
| `Seat(n)` | Own hand, public zones, counts of hidden zones | All builds |
| `Spectator` | Public zones only (optionally delayed) | All builds |
| `Omniscient` | Full state | Local dev builds and tests only; never sent by the server |

- **Visibility per component, not per zone.** A card can be hidden even from its owner (Skyjo), public in count but hidden in content (a Catan hand), or a hidden number (money in Acquire).
- **Opaque identities.** A face-down component carries a fresh opaque ID, never its real identity. When hidden components are shuffled or moved between hidden zones, their opaque IDs are reissued, so nobody can track a card through a shuffle.
- **Leak test.** The conformance suite checks that a seat's view does not change when only information hidden from that seat changes.
- **Dev multiclient.** A developer tool hosts the engine locally and shows several seat views side by side plus the omniscient view, with step-through of the move log. Each seat is driven by hand or by a bot. Later the same tool connects several clients to a real server.
- **Pass-and-play privacy screen.** On a shared device the session inserts a neutral hand-over screen (spectator view) between human seats; the next player confirms before their view is shown. Bot seats need no screen.

## Interaction model

User input is translated into moves by a generic, game-independent selection protocol, so no client can ever build an illegal move.

1. The core describes each legal move as an ordered list of **choices**: pick a component, pick a zone, pick an option, pick a number, pick a subset (e.g. "play Jack" → "wish Hearts"; "keep these dice").
2. Given the choices made so far, the core returns the **next selectable** components, zones and options.
3. The client highlights them and turns taps, clicks, drags or keyboard input into the next choice.
4. When the choices made form a legal move's complete path, it is submitted (optionally after a confirm step); "undo choice" steps back. In trainer mode the confirm step shows the selected move's estimate and its gap to the best move before anything is sent.

The same protocol drives VoiceOver ("selectable: Queen of Hearts"), keyboard play and the text renderer, and lets bots and tests express moves in the same vocabulary as humans.

## Table model & rendering

Clients render a generic **table model**, not game-specific screens; a new game ships assets, layout hints and strings, and needs custom UI only for exceptions.

- **Zones:** draw pile, hand, discard, trick area, tableau, player board, score sheet, board. Each zone has an owner, a visibility and a layout kind (stack, fan, row, grid, free).
- **Boards are typed graphs:** locations of several kinds (squares, hexes, corners, edges, track spaces, territories) with adjacency, per-player paths and layout coordinates; boards may grow without bounds (Carcassonne), so renderers pan and zoom.
- **Components:** card, tile, piece, token, die, marker. Each has a stable ID, a face (or "hidden"), an orientation where relevant (tile rotation, die orientation) and semantic attributes (suit, rank, colour) used for rendering and accessibility.
- **Indicators:** scores, counters, money, turn and phase, announcements, timers.
- **Events → animation.** Renderers animate from core events using stable component IDs; reissued opaque IDs on shuffle are animated as a shuffle. Dice animations only display results that the core already determined.
- **Seat-relative layout.** Positions are abstract ("my hand: bottom", "opponents: clockwise", "trick: centre"). One layout strategy per form factor: browser, Mac window, iPad, iPhone portrait/landscape.
- **Custom widgets** are the escape hatch (e.g. a Skat bidding panel), mounted into named slots of the generic table.
- **Renderers sharing one model:** SwiftUI (with SpriteKit or Canvas for the table surface if animation load requires it), web (DOM/Canvas), and a **text renderer** used by tests and by Claude to check layout and selection logic without a simulator.
- **Themes & decks:** French and German suited decks, colour schemes, card backs, high-contrast and colour-blind-safe sets. Artwork is generated or commissioned under an open licence.

## Bots

The default bot is a generic Information Set Monte Carlo Tree Search (ISMCTS) player that works for every game through the `Game` trait alone; per-game heuristics and exact solvers are optional add-ons.

- **Hidden information:** the bot samples determinizations consistent with its own view; it never reads hidden state.
- **Strength:** controlled by iteration budget and time; optional rollout policies and evaluation heuristics per game. Levels are calibrated by bot-vs-bot rating, not guessed.
- **Personalities (later):** cautious, aggressive, "pub player" with human-like mistakes, deliberate forgetting where perfect memory would spoil the game.
- **Explanations:** the search yields an estimated win probability (or expected score where "win" is ambiguous, e.g. Rommé penalty points) per legal move, with its uncertainty. This powers post-game analysis, coaching hints, the live trainer and puzzles. Estimates come only from the player's own view.
- **Where bots run:** on the device in single player; on the host device or the device of the next human in Game Center matches; on the server in server matches. The `Player` interface is identical everywhere.
- **Performance:** game state must be cheap to clone and apply (compact structs, no heap churn in hot paths). A benchmark crate tracks playouts per second per game.
- **Limits:** real-time games (Nerts) need reaction-time bots, not search; negotiation (trading, auctions) and bidding systems (Bridge) need dedicated logic.
- **Validation:** Connect Four bots are measured against the known perfect solution; dice games against exact optimal strategies.

## Variants & house rules

Each game has one **standard ruleset** and a typed set of options that compose into variants; house rules are configuration, not forks.

- **Standard rulesets** follow an authoritative source where one exists (e.g. International Skat Order, tournament rules of the German Doppelkopf association) and are the reference for tests.
- **Options** are typed and validated (`bool`, enum, bounded number) with declared incompatibilities; an invalid combination is rejected at setup.
- **Presets** are named, shareable option sets ("Thursday group"), serialized with the game ID.
- **Bots** play any valid variant without extra work, because ISMCTS needs only the rules.

## Saves, replays & updates

Rules are not versioned. A game record is `(game ID, variant, seed, move list)` plus the build ID (git commit) that produced it, kept for diagnostics only; saves, replays and network sync share this format. Long matches additionally store snapshots so they need not be replayed from the start.

- **During development** a rule change simply changes behaviour. Golden replays affected by it are re-recorded in the same PR, with a note why.
- **Online operation** uses maintenance windows: running server matches are finished or ended before an update, so no server match ever spans two builds.
- **Client compatibility:** the server accepts only clients whose build is compatible with its own and asks others to update. Because App Store review and user updates lag, rule changes ship together with app releases, and the server may accept the previous app build for a short transition.
- **Asynchronous matches** (Game Center turn-based, later correspondence play) can outlive a release. They carry the build ID; a device with an incompatible build asks the player to update before continuing.
- **Local saves and replays** from older builds are best effort: they load if they still replay to the recorded end state, otherwise they are marked outdated.
- **Upgrade path:** if long-running matches or public archives make it necessary, per-game rule versions can be added later; the build ID in every record keeps that option open.

## Sessions, transports & multiplayer

A **session** runs one match: it owns the state, the RNG and the seats, and talks to the outside world only through a `Transport`. Swapping Game Center for our own server is a transport change, not a game change.

- **Seats** are `Human(local)`, `Human(remote)` or `Bot(level)`; seats can switch at runtime (a bot takes over on disconnect and hands back on return). Seats can be eliminated and become spectators.
- **Transport interface:** submit move, receive view and events, presence, join/leave.

| Transport | Who is authoritative | Hidden info safe | Players online | Stage |
| --- | --- | --- | --- | --- |
| Local (single device, pass-and-play) | Device | Yes, with a privacy screen | Same device | Core |
| Dev multiclient | Local engine | N/A (debug) | Same machine | Core |
| Game Center turn-based | Every client holds full state | No (trust between friends) | Asynchronous | Mac |
| Game Center real-time | Host device | No (host sees all) | Simultaneous | Mac |
| Own server (WebSocket) | Server | Yes | Both | Web |

- **Game Center turn-based** has one active participant at a time and a limited match-data size. Simultaneous decisions are serialized, with each choice sealed in the match data; bots are computed by the device whose turn comes next. Real-time games need Game Center real-time or the own server.
- **Game Center is a convenience tier for friends**, not a security boundary. Competitive or public play requires the own server.

## Server

The server is a Rust service (e.g. axum on tokio) that links the same game crates as the clients and is the single source of truth for server matches.

- **Responsibilities:** match lifecycle, shuffling and chance, per-seat views, bot seats, reconnection, spectators, client build check, arrival order for real-time games.
- **Protocol:** WebSocket with a versioned, language-neutral schema (moves, views, events, presence). Any client in any language can play. Version 1 (JSON) is described in [design/server.md](design/server.md).
- **Starts small:** the dev server used by the web debug client in the core stage grows into this service. First public version = rooms joined by code, no accounts.
- **Later:** accounts (Sign in with Apple, passkeys), friends, lobby, matchmaking, ratings, tournaments, server-side deep analysis.
- **Cost discipline:** bots run on devices by default; server-side bots only for server matches.
- **Self-hostable:** one binary plus a database (SQLite first, Postgres when needed), configured by environment variables, documented for community hosting.

## Clients & platforms

All clients are thin: they render the table model, run the selection protocol and forward choices. Game logic, bots and table models come from the Rust core.

| Client | Core binding | Role | Stage |
| --- | --- | --- | --- |
| CLI runner | Native Rust | Play, replay, simulate, bot-vs-bot, text renderer; main tool for Claude | Core |
| Web dev multiclient | Dev server (later Wasm) | Several seat views + omniscient view, move log scrubbing | Core |
| Web client | Wasm (offline, bots) + WebSocket (online) | Public browser client | Web |
| macOS app | UniFFI → Swift (XCFramework) | Native SwiftUI | Mac |
| iPadOS app | same as macOS | Touch and Pencil layout, table mode | iPad |
| iOS app | same as macOS | Compact layouts, portrait | iPhone |
| Android (option) | UniFFI → Kotlin | Compose UI | Later |

- **One SwiftUI codebase** for macOS, iPadOS and iOS with per-form-factor layout strategies; no Mac Catalyst.
- **The FFI boundary is narrow and data-only:** create session, submit choice, get selectable items, get table model and events.
- **Apple extras** (Game Center, SharePlay, widgets, Live Activities) live in the Swift layer behind the `Transport` and notification abstractions.

## Testing & conformance suite

A game is done when it passes the generic conformance suite plus its own rule tests; the suite is the architect's main acceptance instrument.

**Conformance suite** — runs thousands of random and bot-driven games per game and variant, checking after every move:

- **Determinism:** same config, seed and moves → identical state hash, on every target (native, Wasm).
- **Legality:** every move from `legal_moves` applies; every other move is rejected; `pending` is never empty before the game ends.
- **Termination:** every game reaches `outcome` within a declared move bound.
- **No leaks:** a seat's view is unchanged when only information hidden from that seat changes; opaque IDs are reissued on hidden shuffles.
- **Round trips:** state, moves, views and records serialize and deserialize losslessly.
- **Invariants:** game-declared conservation laws (e.g. 32 cards always accounted for).
- **Selection protocol:** every legal move is reachable through choices, and no choice path leads to an illegal move.

**Per game:** rule tests written from the rules specification, golden replays from hand-played example games, and property tests for scoring.

**Bots:** regression ratings in CI (a new bot must not lose strength), playout benchmarks.

**Clients:** text-renderer snapshot tests for every game; SwiftUI previews and snapshot tests; web end-to-end tests driving the dev multiclient.

## Localization & accessibility

English is the source language and German the first translation; full screen-reader play is a core feature, not an afterthought.

- Fluent (`.ftl`) message files in `/locales`, one per crate or game, shared by Rust, Swift and web; plural and gender rules handled by the format. Rust clients format IDs with the `l10n` crate, which the deterministic crates never depend on. CI fails on keys missing in `en` or `de` and on message IDs declared by code without an `en` message ([design/localization.md](design/localization.md)).
- Game names, card names, rules texts and bot explanations are localized; internal IDs (`mau-mau`, `queen-of-hearts`) never are. Regional terminology per locale where games demand it (German suit names for German-suited decks). Language-dependent game content (e.g. word lists) is game data, not UI text.
- The semantic table model and the selection protocol generate VoiceOver labels, hints and rotor actions automatically ("Queen of Hearts, in your hand, playable"); events are announced ("Anna plays Ace of Spades; trick to Anna").
- Every game is fully playable by keyboard and by screen reader. Dynamic Type, high-contrast and colour-blind-safe decks, reduced motion, haptics for turn changes.
- Testing with blind and visually impaired players is planned from the Mac stage on, ideally with a partner organization.

## Repository layout

```text
/crates
  game-core/        Game trait, table model, selection protocol, RNG, records
  games/<game>/     one crate per game
  bots/             ISMCTS and heuristics
  session/          seats, transports interface, match flow
  conformance/      generic suite (+ rock-paper-scissors test game)
  registry/         the games front ends pick by ID (CLI, server, bindings)
  server/           axum service (dev server first), WebSocket protocol
  cli/              runner, simulator, text renderer
  l10n/             Fluent catalog loader and formatter, localization check
  ffi/              UniFFI bindings
  wasm/             wasm-bindgen package
/apps
  apple/            SwiftUI app (macOS, iPadOS, iOS)
  web/dev/          dev multiclient page, served by the dev server
  web/              later the web client
/docs               vision, architecture, design/, games/<game>/rules.md
/locales            en (source), de, ...
```

The working name appears only in the crate prefix, the bundle ID and one display-name constant, so renaming is a single PR.
