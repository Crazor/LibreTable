# 0001: Game trait, selection protocol and table model types

- **Status:** proposed
- **Date:** 2026-11-26
- **Owner:** Architect

## Context

Every game, bot, renderer and transport depends on these interfaces ([architecture: game core](../architecture.md#game-core-the-game-trait), [interaction model](../architecture.md#interaction-model), [table model](../architecture.md#table-model--rendering)). Changing them later is expensive, so they are validated against deliberately different games before being accepted.

## Decision (proposal)

- The trait shape sketched in the architecture document: `setup`, `pending` (`All(seats) | Any(seats) | Chance | Finished`), `legal_moves`, `apply` returning events, `view(observer)`, `table(view)`, `outcome`.
- Visibility is defined per component; face-down components carry opaque IDs that are reissued on hidden shuffles.
- Moves are composed through a choice protocol (component, zone, option, bounded number, subset).
- Teams/partnerships are part of `Outcome` and view rules.

Refinements from implementing `game-core` (see [design/game-core.md](../design/game-core.md)), proposed by Claude for the architect to confirm:

- **Events are an associated type** `Game::Event`, not one fixed core enum. What a game reports (trick won, wish declared) is game-specific; generic animation events in terms of table components are derived once the table model exists.
- **Explicit chance steps** go through a separate `apply_chance(state, rng)` called while `pending` is `Chance`. Its default panics, so games without chance steps do not implement it. Chance inside a move still uses the `rng` passed to `apply`.
- **`All(seats)` is applied one move at a time.** The game keeps sealed moves in its state and resolves when the last seat has submitted, so a record is always a flat list of `(seat, move)` in arrival order.
- **`SeatSet` is a 64-bit mask**, which caps a match at 64 seats and keeps `pending` allocation-free in bot playouts.
- **`GameRng` is ChaCha12 (`chacha12-v1`)**, implemented in the crate without new dependencies, so players cannot predict future draws from the cards they see. Changing the algorithm changes every replay and needs an ADR.
- **`Outcome`** v0: per seat `{ rank, score: i64 }`, rank 1 = best and ties share, plus `teams` as a partition of the seats.
- `table(view)` is added together with the table model (milestone 1, issue 5).

## Consequences

- Validate with Mau-Mau, Rommé, Connect Four and the rock-paper-scissors test game before acceptance.
- Requirements from [reference games](../reference-games.md) must be expressible without breaking changes, even if not implemented yet.

## Open points

- Besides composing a move from choices, allow submitting a **draft** (e.g. a rearranged table in tile rummy) that the core validates.
- Lazy enumeration for very large move spaces; how bots sample moves there.
- Where per-game history (repetition) lives and how it is hashed.
- Exact shape of `Outcome` for teams, eliminations and multi-round scoring (v0 above; to be checked against Doppelkopf, Rommé penalty points and elimination games).
