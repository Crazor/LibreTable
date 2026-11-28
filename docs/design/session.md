# session and bots

The `libretable-session` crate runs one match; `libretable-bots` holds the `Player` interface and the bots. Both are `no_std` + `alloc` and have no I/O, clock, threads or global randomness, so the CLI, the dev server and later the apps share the same match logic. Overview: [architecture: sessions, transports & multiplayer](../architecture.md#sessions-transports--multiplayer) and [bots](../architecture.md#bots).

## `Session<G>`

A session owns the authoritative state, the match's `GameRng` and the `GameRecord` it writes. Front ends drive it step by step and never see the state: only views, table models, choices, events and the record leave it.

| Function | Purpose |
| --- | --- |
| `new(config, seats, seed, build) -> Result<Self, InvalidVariant>` | Validates the variant, sets up from the seed, resolves initial chance steps. |
| `from_record(record) -> Result<Self, ReplayError>` | Replays a record (via `game_core::replay`) and continues from its end. |
| `seats()`, `pending()`, `outcome()` | Match facts. `pending()` is never `Chance`: the session resolves chance steps itself after setup and after every move. |
| `view(observer)`, `table(observer)` | What one observer may know, as view or table model. |
| `prompt(seat, made) -> Result<Prompt, SubmitError>` | Selection protocol for clients: the next selectable choices after the choices `made` so far, and whether they already form a complete move. |
| `submit_choices(seat, path) -> Result<Vec<Event>, SubmitError>` | Applies the move whose whole choice path is `path`, records it, resolves following chance steps and returns the events. |
| `decision(seat) -> Option<Decision<View>>` | What a `Player` gets: the seat's own view and the choice paths of its legal moves. |
| `play(seat, &mut dyn Player<G>)` | `decision` → `Player::decide` → `submit_choices`. |
| `submit_move(seat, move)` | Trusted sources that already hold moves (records, tests); bypasses the selection protocol. |
| `record()`, `into_record()` | The record so far. |

- **Stateless selection.** The client keeps the choices it made (undo is dropping the last one) and asks `prompt` again after each; the session rebuilds the `Selection` from the state every time. That keeps the session free of per-client state and suits a request/response transport. Clients never receive `Selection::remaining()` moves, which may carry hidden details.
- **`All` and `Any`.** The session accepts a move from any pending seat in arrival order; the order it applies them is the order in the record. Front ends decide whom to ask: the CLI asks the lowest pending seat first.
- **Errors** (`SubmitError`): `NotPending(seat)`, `NotSelectable { index }`, `Incomplete`, `Ambiguous(SelectionError)`, `Illegal(IllegalMove)`. A rejected call leaves the session unchanged.
- **Equality.** Two sessions are equal when state, generator and record are; a session restored with `from_record` equals the original. `Debug` prints only game, seats, move count and pending, never the state.
- **Events of chance steps** resolved in `new` are not reported (no game uses an initial chance step yet; Mau-Mau deals in `setup`, which reports no events).
- `derive_seed(&mut GameRng) -> Seed` draws a fresh 256-bit seed, so one master seed can determine a match seed and one seed per bot.

Not yet: seats as `Human(local) | Human(remote) | Bot(level)` and bot takeover, timeouts, transports, snapshots. They will be built around this API. The dev server already keeps a driver (human or bot) and the human's draft per seat around a session, outside it ([server](server.md)).

## `Player` and `RandomBot`

```rust
pub struct Decision<V> { pub seat: Seat, pub view: V, pub paths: Vec<Vec<Choice>> }

pub trait Player<G: Game> {
    fn decide(&mut self, decision: &Decision<G::View>) -> Vec<Choice>;
}
```

- A player answers with one of `decision.paths`, the choice paths of the seat's legal moves in the game's order. Paths are described against the seat's own view, so they reveal nothing a human at that seat could not find by exploring the selection protocol.
- The session builds decisions from `Observer::Seat(seat)` only; a bot never sees `Observer::Omniscient` or the state.
- `RandomBot::new(seed)` picks a path uniformly with its own `GameRng`. It is deterministic for a seed and a sequence of decisions, so bot matches replay exactly.
- The ISMCTS bot (issue 9) implements the same trait. It will need to sample states consistent with a view; the interface for that (a `Game` function or a per-game sampler) is decided there.

Tests: `crates/session/tests/session.rs` (selection, rejection, determinism, record round trip and resume), `crates/bots/tests/random.rs`.
