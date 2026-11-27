# LibreTable

> Working name. The final project name is decided in [ADR-0008](docs/adr/0008-project-name.md).

An open-source platform for turn-based card, dice and board games: implement a game once, play it everywhere — natively on Apple platforms, in the browser and later elsewhere, online with friends and against AI players that can also coach you.

## Status

Pre-alpha; see [Milestone 1](docs/milestones/milestone-1.md). The command-line runner plays the rock-paper-scissors test game:

```sh
cargo run -p libretable-cli -- games
cargo run -p libretable-cli -- play rock-paper-scissors --seats human,bot
cargo run -p libretable-cli -- play rock-paper-scissors --seats bot,bot --seed 7 --record match.json
cargo run -p libretable-cli -- replay match.json --seat 0
cargo run -p libretable-cli -- simulate rock-paper-scissors -n 1000 --seats 3
cargo run -p libretable-cli -- --lang de games
```

See [docs/design/cli.md](docs/design/cli.md) for all options.

## Documentation

| Document | Contents |
| --- | --- |
| [docs/vision.md](docs/vision.md) | Purpose, goals, roadmap, game selection, differentiating features, licensing and funding |
| [docs/architecture.md](docs/architecture.md) | Principles, layers and core concepts |
| [docs/design/](docs/design/) | Detailed design per component (grows with the code) |
| [docs/adr/](docs/adr/) | Architecture decision records |
| [docs/games/](docs/games/) | Rules specification per game |
| [docs/reference-games.md](docs/reference-games.md) | What well-known games demand from the engine |
| [docs/policies.md](docs/policies.md) | Privacy, legal and App Store constraints |

## Contributing

Every commit must carry a [Developer Certificate of Origin](https://developercert.org/) sign-off matching its author, which keeps relicensing options open ([ADR-0002](docs/adr/0002-licences.md)). Sign off with `git commit -s`; fix an existing branch with `git rebase --signoff main`. CI rejects pull requests with unsigned commits; run `scripts/check-dco.sh main..HEAD` to check locally.

## Licence

See [ADR-0002](docs/adr/0002-licences.md) (legal review pending before the first public release):

| Part | Licence | Text |
| --- | --- | --- |
| Everything except the server: core, games, bots, session, CLI, bindings, apps | MPL-2.0 | [LICENSE-MPL-2.0](LICENSE-MPL-2.0) |
| Server | AGPL-3.0-or-later | [LICENSE-AGPL-3.0](LICENSE-AGPL-3.0) |
| Artwork, decks | CC BY-SA 4.0 | [LICENSE-CC-BY-SA-4.0](LICENSE-CC-BY-SA-4.0) |

Each crate declares its SPDX licence in `Cargo.toml`; `cargo deny check` enforces compatible dependency licences.
