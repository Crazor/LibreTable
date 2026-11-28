# bots

The `libretable-bots` crate holds the `Player` interface and the bots. It is `no_std` + `alloc`: no I/O, clock, threads or global randomness; a bot's randomness comes from its own `GameRng`, seeded by the front end (`session::derive_seed`), so bot matches replay exactly.

## Player

`Player<G>::decide(&mut self, &Decision<G::View>) -> Vec<Choice>`. A `Decision` is the deciding seat, its own view (`Observer::Seat`) and the choice paths of its legal moves; the bot answers with one of the paths. Bots never see the state and the session never builds a decision from `Observer::Omniscient` ([architecture: bots](../architecture.md#bots)).

- `RandomBot` picks a path uniformly.
- `IsmctsBot` searches (below). It plays every game that implements `Determinize`.

## Determinizations

A search bot has to play moves forward, which needs a full state, but it only has a view. The view type is game-specific, so only the game can turn it into a state. Games that want search bots implement the optional trait `game_core::Determinize`:

```rust
fn determinize(view: &Self::View, seat: Seat, rng: &mut GameRng) -> Self::State;
```

It fills in everything hidden from `seat` at random, uniformly among the possibilities consistent with the view unless the game documents otherwise. Its only input is the view, so it cannot leak. The conformance suite checks, at every decision point and for every pending seat, that the sampled state has the same view for that seat, the same pending seats and the same legal moves for it (`Suite::determinize`, see [conformance](conformance.md)).

A view must therefore carry every public fact the match needs to go on, even ones the table does not show. Mau-Mau's view carries a `Flow` (who called "last card", whether a missed or false call is open to be caught, passes and turns so far), the round results and the seat's own just-drawn card.

Mau-Mau and rock-paper-scissors implement `Determinize`; a game without it can still be played by humans and the random bot. The registry marks such games `searchable`, and front ends offer the ISMCTS bot only for them.

## ISMCTS

`IsmctsBot` is single-observer Information Set Monte Carlo Tree Search (Cowling, Powley and Whitehouse 2012), generic over `G: Determinize`. `IsmctsConfig` sets the iterations per decision (default 500), the UCB1 exploration constant (0.7) and a playout cut-off (20 000 moves).

Each iteration:

1. Samples a determinization of the view.
2. Walks the tree from the root along moves legal in that determinization. Nodes are keyed by `(seat, move)`, so one tree is shared across determinizations. Selection uses UCB1 with availability counts: a child's exploration term uses how often its move was legal, not the parent's visits.
3. Adds one node.
4. Plays random moves to the end.
5. Backs up the reward of each node's mover.

Rewards come from the outcome's ranks: rank 1 scores 1, the last rank 0, linear in between. A cut-off playout scores ½ for everybody.

The bot answers the root move visited most; `search` returns every root move with visits and mean reward, the base for later explanations (win estimates per move).

- Chance steps (`Pending::Chance`) are resolved with the bot's generator; randomness inside moves (draws) also uses it, so the same move may lead to different states in different iterations.
- Simultaneous decisions (`All`) are searched as if the lowest pending seat moved first.
- `no_std` has no `ln` and `sqrt`; the crate computes them itself (series and Newton's method).

**Memory.** Decided with the architect for v1: the bot remembers nothing between decisions. It samples from the current view only, so cards played earlier and now buried in the discard pile count as unseen. Memory of past views is a later strength and difficulty lever.

## Strength and speed

- `tests/ismcts.rs`: ISMCTS with 150 iterations against `RandomBot` in two-player Mau-Mau, 40 matches with alternating seats. A win is rank 1 alone. It must win with one-sided binomial p < 0.01 against an even split; it won 34 of 40 (p ≈ 4·10⁻⁶). Also: determinism and simultaneous play.
- `benches/playouts.rs` (`cargo bench -p libretable-bots`): random playouts and moves per second, and ISMCTS iterations per second, per game. On an Apple-silicon laptop: Mau-Mau about 23 000 random playouts/s (2 seats) and 25 000 ISMCTS iterations/s, so a 500-iteration decision takes about 20 ms.

## Front ends

- CLI: seat kinds `human`, `bot` (random, also `random`) and `ismcts`; `table simulate --bots ismcts,bot`; `--iterations` sets the budget ([cli](cli.md)).
- Dev server: driver `"ismcts"` with 300 iterations, offered when `welcome.games[].ismcts` is true ([server](server.md)).

## Open points

- Memory of past views, and levels calibrated by bot-vs-bot rating.
- Rollout policies and evaluation heuristics per game.
- Time budgets in addition to iteration counts; that needs the clock outside the bot.
- The bot runs inside the dev server's request handling; a long search should move off the async runtime.
