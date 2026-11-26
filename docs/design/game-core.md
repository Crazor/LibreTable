# game-core

The `libretable-game-core` crate holds the `Game` trait and the types every other crate shares. It is `no_std` + `alloc`, `#![forbid(unsafe_code)]`, and depends only on `serde`. Interface decisions are in [ADR-0001](../adr/0001-game-interface.md); this document describes what is implemented.

Not yet here: `table(view) -> TableModel` and the selection protocol (milestone 1, issue 5).

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

Associated types `Config: Variant`, `State`, `Move`, `View` and `Event` are all `Clone + Debug + Eq + Serialize + DeserializeOwned`, so the conformance suite can compare and round-trip them. Functions: `setup`, `pending`, `legal_moves`, `apply`, `apply_chance`, `view`, `outcome`.

- **Simultaneous moves (`All`).** `apply` is still called once per seat in arrival order. The game keeps sealed moves in its state, drops each submitted seat from the pending set and resolves when the last move arrives. Views hide sealed moves until then.
- **Chance.** Randomness inside a move (drawing from a shuffled pile) uses the `rng` passed to `apply`. A chance step on its own (dealing, a die roll before anyone acts) is `Pending::Chance`, resolved by `apply_chance`. The default `apply_chance` panics, so only games that use `Pending::Chance` implement it.
- **Events** are a per-game type for now. Generic animation events in terms of table components come with the table model.

## `GameRng` (`chacha12-v1`)

ChaCha with 12 rounds, keyed by the seed. The block function is checked against the RFC 8439 test vector (20 rounds), and the 12-round output is pinned against the published all-zero-key keystream. The exact output order, the unbiased `below(bound)` (Lemire) and the Fisher–Yates `shuffle` are documented in `rng.rs`. Any change to them changes every replay and needs an ADR. ChaCha rather than a small PRNG keeps future draws unpredictable to players who see dealt cards.

`GameRng` serializes key, block counter and position, so snapshots resume mid-stream. `Debug` hides the key.

## Records and replay

`GameRecord { schema, game, build, variant, seats, seed, moves }` with `moves: Vec<{ seat, move }>` in applied order. Chance steps are not recorded; they follow from the seed. `schema` is `RECORD_SCHEMA_VERSION` (ADR-0003). `Seed` is 64 hex digits in JSON and 32 bytes in binary formats.

`replay::<G>(&record)` is the reference match loop: it validates the variant, sets up from the seed, resolves every `Chance` (also after the last move), checks each move's seat is pending and applies it. It returns the state and the generator, ready to continue.
