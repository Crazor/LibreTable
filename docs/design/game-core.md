# game-core

The `libretable-game-core` crate holds the `Game` trait and the types every other crate shares. It is `no_std` + `alloc`, `#![forbid(unsafe_code)]`, and depends only on `serde`. Interface decisions are in [ADR-0001](../adr/0001-game-interface.md); this document describes what is implemented.

## Types

| Type | Meaning |
| --- | --- |
| `Seat` | Seat index `0..64`, serialized as a number. |
| `SeatCount` | Seats in a match, `1..=64`. |
| `SeatSet` | Bit mask of seats; `Copy`, no allocation. |
| `Observer` | `Seat(seat)`, `Spectator`, and `Omniscient` only with the `dev` feature. Games ask `seat()`, `sees_private_of(owner)` and `sees_all()` instead of matching on it (ADR-0001). |
| `Pending` | `All(SeatSet)`, `Any(SeatSet)`, `Chance`, `Finished`. |
| `Outcome` | Per seat a `SeatResult { rank, score }` (rank 1 = best, ties share), plus `teams` as a partition of the seats (empty without partnerships). |
| `GameId` | Lowercase kebab-case ID, checked at compile time when used in a `const`. |
| `Variant` | Trait for a game's config; `Default` is the standard ruleset, `validate(seats)` rejects invalid combinations. |
| `IllegalMove`, `InvalidVariant` | Errors carrying a Fluent message ID. |
| `Seed`, `GameRng` | 256-bit seed and the generator (below). |
| `GameRecord<C, M>`, `RecordOf<G>` | Saved match (below). |

## The `Game` trait

Associated types `Config: Variant`, `State`, `Move`, `View` and `Event` are all `Clone + Debug + Eq + Serialize + DeserializeOwned`, so the conformance suite can compare and round-trip them. Functions: `setup`, `pending`, `legal_moves`, `apply`, `apply_chance`, `view`, `table`, `choices`, `outcome`.

- **Simultaneous moves (`All`).** `apply` is still called once per seat in arrival order. The game keeps sealed moves in its state, drops each submitted seat from the pending set and resolves when the last move arrives. Views hide sealed moves until then.
- **Chance.** Randomness inside a move (drawing from a shuffled pile) uses the `rng` passed to `apply`. A chance step on its own (dealing, a die roll before anyone acts) is `Pending::Chance`, resolved by `apply_chance`. The default `apply_chance` panics, so only games that use `Pending::Chance` implement it.
- **Events** are a per-game type for now. Generic animation events in terms of table components are derived later from the table model.

## Table model (v0)

`table(view) -> TableModel` turns a view into what every renderer draws. It is built from the view, so it cannot show more than the view.

| Type | Meaning |
| --- | --- |
| `TableModel` | `zones` and `indicators`; `zone(id)` and `component(id)` look things up. |
| `Zone` | `id: ZoneId`, `kind` (draw pile, hand, discard, trick, tableau, player board, score sheet, board, other), `label` (Fluent ID), `owner: Option<Seat>`, `visibility` (public, owner, hidden), `layout` (stack, fan, row, grid with `columns`, free), `components` bottom to top / left to right. |
| `Component` | `id: ComponentId`, `kind` (card, tile, piece, token, die, marker), `face`, `rotation` in degrees. |
| `Face` | `Hidden`, or `Shown { key, attributes }` with an internal key (`"queen-of-hearts"`) and semantic `Attribute { name, value }` pairs (suit, rank, colour). |
| `Indicator` | `kind` (score, counter, money, turn, phase, announcement), `label`, `owner`, `value` (number, seat or Fluent ID). |

`ComponentId` and `ZoneId` are `u32` newtypes. Face-up components keep their ID; face-down ones carry opaque IDs the game reissues on hidden shuffles. Seat-relative placement is derived by renderers from zone kind and owner. Not in v0: typed board graphs (locations, adjacency, paths) and addressable empty cells, layout hints, custom-widget slots, timer indicators and orientations beyond `rotation`. Until boards exist, a placement game (Connect Four) models a board as a `Layout::Grid` zone filled with placeholder components for empty cells, or as one zone per column.

## Selection protocol

`choices(view, move) -> Vec<Choice>` describes a legal move as the path of choices that selects it, against the moving seat's view. `Choice` is `Component(ComponentId)`, `Zone(ZoneId)`, `Option(String)` (a Fluent ID such as `"suit-hearts"`), `Number(i64)` or `Subset(ComponentSet)`. A `ComponentSet` is sorted and duplicate-free, so a subset compares equal whatever order it was picked in; a client offers its members for toggling and submits the set. JSON: `{"component":3}`, `{"option":"suit-hearts"}`, `{"subset":[2,4]}`.

For one seat at one point the paths must be unique and prefix-free; then a path equal to the choices made identifies the move. A single legal move may have an empty path.

`Selection<M>` drives the protocol generically from `legal_moves` and `choices`: `selectable()` returns the distinct next choices in move order, `choose(choice)` rejects anything not selectable, `undo()` steps back, `complete()` / `into_move()` yield the move once the choices made equal its whole path. The session runs it on the state; clients receive selectables and send choices, never `remaining()` moves, which may carry hidden details.

`check_table(table, seat)` checks a table as seen by a seat (or a spectator): zone and component IDs are unique, and no face is shown in a hidden zone or in another seat's owner-only zone.

`check_selection::<G>(state, seat, rng)` is the conformance check: the seat's table passes `check_table`, paths are unique and prefix-free, no subset is empty, every component and zone they name is in the seat's table, walking the whole choice tree reaches every legal move exactly once, and each reached move applies to a copy of the state. The conformance harness runs it for every seat at every decision point, and `check_table` for every seat's and the spectator's table.

## `GameRng` (`chacha12-v1`)

ChaCha with 12 rounds, keyed by the seed. The block function is checked against the RFC 8439 test vector (20 rounds), and the 12-round output is pinned against the published all-zero-key keystream. The exact output order, the unbiased `below(bound)` (Lemire) and the Fisher–Yates `shuffle` are documented in `rng.rs`. Any change to them changes every replay and needs an ADR. ChaCha rather than a small PRNG keeps future draws unpredictable to players who see dealt cards.

`GameRng` serializes key, block counter and position, so snapshots resume mid-stream. `Debug` hides the key.

## Records and replay

`GameRecord { schema, game, build, variant, seats, seed, moves }` with `moves: Vec<{ seat, move }>` in applied order. Chance steps are not recorded; they follow from the seed. `schema` is `RECORD_SCHEMA_VERSION` (ADR-0003). `Seed` is 64 hex digits in JSON and 32 bytes in binary formats.

`replay::<G>(&record)` is the reference match loop: it validates the variant, sets up from the seed, resolves every `Chance` (also after the last move), checks each move's seat is pending and applies it. It returns the state and the generator, ready to continue.
