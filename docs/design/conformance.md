# conformance

The `libretable-conformance` crate will hold the generic conformance suite (milestone 1, issue 4) described in [architecture: testing & conformance suite](../architecture.md#testing--conformance-suite). It is a development tool and never shipped.

## Rock-paper-scissors test game

`conformance::rock_paper_scissors` exercises simultaneous decisions (`Pending::All`), which no shipped game in milestone 1 needs. The rules are in the module documentation; in short:

- 2 to 64 seats; variant `rounds` (at least 1, default 3).
- Each round every pending seat throws once, in any arrival order. The game keeps the throws sealed in its state, removes each seat that threw from the pending set, and resolves when the last seat has thrown.
- Exactly two different shapes: every seat with the winning shape wins the round. One shape or all three: draw. Draws count as rounds.
- Score = rounds won; rank by score, ties share. A match takes exactly `seats × rounds` moves, no chance step.
- A seat's view shows only its own sealed throw; spectators see who has thrown, not what. The omniscient view (`dev` feature) shows all.

Rule tests are in `crates/conformance/tests/rock_paper_scissors.rs`.
