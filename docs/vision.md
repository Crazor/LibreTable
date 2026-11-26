# Vision

We build an open-source platform on which any turn-based card, dice or board game can be implemented once and played everywhere: natively on Apple platforms, in the browser, and later on other platforms, online with friends and against AI players. A particular aim is excellent implementations of games that so far lack good software (German card, dice and pub classics), rather than one more chess or Go server.

## Goals

- **One rules engine, many games.** Every game is a deterministic rules module behind one common interface. Clients, bots, transports and the server know the interface, never a specific game.
- **AI players for every game.** Generic search-based bots (ISMCTS) fill empty seats, take over for disconnected players and serve as training partners. They play any game that implements the interface.
- **Native first.** SwiftUI apps for macOS, iPadOS and iOS; a web client for development and as a platform of its own.
- **Open by default.** Code, rules modules and tools are open source; the project brand is protected so that the community, not store clones, benefits.
- **Built by agents, governed by an architect.** Claude writes the code; the human owner sets interfaces, invariants, rules and acceptance criteria. Correctness is checked by machines (conformance suite, replays, text renderer), not only by eye.

## Roadmap

The platform grows in five stages; each stage ends at a gate that is playable, not just built.

```text
1 Core     Rust core, first games, ISMCTS bots, CLI, conformance suite, web dev multiclient
           gate: three games pass conformance, bots beat random play
2 Web      Public web client (Wasm + WebSocket), minimal server with rooms joined by code
           gate: friends play online in the browser
3 Mac      SwiftUI app via UniFFI, Game Center transport, keyboard and VoiceOver play
           gate: every game playable by keyboard and VoiceOver
4 iPad     Touch layouts, pass-and-play, SharePlay
           gate: touch layouts for all games
5 iPhone   Compact layouts, widgets, Live Activities, App Store release
```

The web dev multiclient is a tool of the core stage and becomes the public web client in stage 2.

## Game selection

Early games are chosen to stress the core rather than the rules, and from games the architect knows well enough to sign off rule tests.

1. **Mau-Mau** — walking skeleton through all layers; hidden hands, draw pile reshuffle, multi-step moves (Jack + wish), many house rules.
2. **Rommé** — melding and laying off, open table zones, taking the discard, jokers; multi-card moves stress the selection protocol; many house rules (e.g. minimum first meld).
3. **Connect Four** — perfect information on a board; proves the interface is generic; bot strength is measurable against the known solution.

A tiny **rock-paper-scissors** test game in the conformance crate (not shipped) exercises simultaneous `All(seats)` decisions.

**Next:** Sechsundsechzig (2 players: tricks, trump, melds of 20/40, closing the stock) and Arschloch/President (rank-based card exchange between rounds) as the first trick-taking and multi-round games; then Skat once the architect has refreshed it; then board games, dice games and mixed card/board games.

**Wanted later:**

- **Canasta and Samba-Canasta** — partnerships, shared team melds, frozen discard pile; builds on the Rommé modules.
- **Doppelkopf and Bridge** — the architect wants to learn both: a real test of the trainer mode.
- **Tile rummy (Rummikub-style)** — a turn may rearrange every meld on the table. Rummikub is a trademarked product, so we implement tile rummy under our own name; the traditional Turkish Okey is a public-domain relative.
- **Ligretto** — the real-time test case for `Any(seats)`: everyone plays at once without turns. Ligretto is a Schmidt Spiele product, so we implement its public-domain ancestor **Nerts** (also called **Pounce**) under our own name and artwork.
- **Dice games** — Kniffel (public domain as Yacht/Generala), Schocken, Meiern (Mäxchen), Zehntausend (Farkle), Shut the Box and others; exact optimal play makes them ideal for the trainer.

Only public-domain games or games with a publisher licence are implemented. Commercial games serve as [reference games](reference-games.md) for requirements only.

## Differentiating features

Three features set the platform apart and follow almost directly from the architecture: bot analysis, house-rule variants and full screen-reader play. The rest is backlog, ordered by expected impact.

| Feature | What it is | Builds on | Priority |
| --- | --- | --- | --- |
| Post-game analysis & coach | Win probability per move, "better was…", hints on demand | ISMCTS statistics | High |
| Live trainer | Two modes. *Overview:* the estimate for every legal move while you choose. *Check my move:* you select a move without sending it and see its estimate plus how much stronger the best move would be (e.g. "−12 % vs. best"); then you send it or change your mind. Updates as the search runs; differences within the search's uncertainty are shown as "about equal"; optionally only the gap is shown, not which move is best. Solo and bot games only; in games with humans only by table agreement and marked unranked | ISMCTS statistics, selection protocol | High |
| House-rule presets | Composable, shareable variants per group | Variant system | High |
| Screen-reader play | Full VoiceOver and keyboard play for every game | Table model, selection protocol | High |
| Seamless bot takeover | Bot fills empty or dropped seats and hands back | Seats, sessions | High |
| SharePlay game night | Play together during a FaceTime call | Transports (Apple) | Medium |
| Puzzles from real games | "Doppelkopf problem of the day" | Records, analysis | Medium |
| Bot personalities & ratings | Cautious, aggressive, human-like (including deliberate forgetting, e.g. Memory, card counting); calibrated levels | Bots, rating | Medium |
| Widgets & Live Activities | "Your turn in Skat with Jan" | Async matches | Medium |
| Game factory | Rules in prose → game module → conformance green; later user-made variants | Game trait, suite | Medium |
| Companion for physical play | Score sheets, group stats, leagues | Records | Low |
| Bot at a physical table | Camera recognizes played cards | Vision, bots | Low |

## Licensing & funding

All code is open; the brand is protected; money comes from donations first, and only server features that cost money are ever paid. Licences are decided in [ADR-0002](adr/0002-licences.md).

- Donations with a public cost sheet, as Lichess does.
- Free App Store app with an optional tip in-app purchase.
- Later, a supporter subscription for server-side features only (deep analysis, tournaments, long archives). Games, bots and on-device analysis stay free forever.
- Paid modules only for licensed publisher games, where licence fees apply anyway.
- Cost discipline: bots run on devices by default, so the server mostly relays and validates.
