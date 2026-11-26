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

## Consequences

- Validate with Mau-Mau, Rommé, Connect Four and the rock-paper-scissors test game before acceptance.
- Requirements from [reference games](../reference-games.md) must be expressible without breaking changes, even if not implemented yet.

## Open points

- Besides composing a move from choices, allow submitting a **draft** (e.g. a rearranged table in tile rummy) that the core validates.
- Lazy enumeration for very large move spaces; how bots sample moves there.
- Where per-game history (repetition) lives and how it is hashed.
- Exact shape of `Outcome` for teams, eliminations and multi-round scoring.
