# 0010: WebSocket protocol v1

- **Status:** proposed
- **Date:** 2026-11-27
- **Owner:** Claude proposes

## Context

The dev server ([ADR-0005](0005-web-dev-client.md)) is the seed of the real server, and its WebSocket protocol is the first wire format of the project ([architecture: server](../architecture.md#server)): "versioned, language-neutral schema … any client in any language can play". The web dev multiclient is its first client; the SwiftUI apps and the public web client will follow. [ADR-0003](0003-encoding.md) proposes a compact binary encoding for the wire and JSON for debugging.

## Decision (proposal)

- **JSON text frames, one message per frame.** Every message is an object `{"v": 1, "type": "<kebab-case>", ...fields}`. The version is checked before anything else; another version is rejected with an error naming the expected one. JSON is the only encoding of v1: the dev server is a debugging tool, browsers read JSON natively, and the binary encoding of ADR-0003 can be added as a second encoding of the same messages later.
- **Text is never prose.** Labels, options, reasons and error codes are Fluent message IDs. On `hello` the client names a locale; `welcome` carries every message the server or a registered game can send, formatted for that locale with arguments left as `{$name}` for the client to fill in. Clients need no Fluent implementation; page messages therefore use no selectors (plurals), which v1 cannot select on the client.
- **The selection protocol on the wire.** Clients send single choices (`choose`), `undo` and `submit` for a seat; the server keeps each seat's draft and sends `prompts` (choices made, selectable, complete) in every `state`. Clients never send moves and never receive another seat's legal moves. Drafts live in the server's match host, not in the session, which stays stateless about selection.
- **State, not deltas.** After every change and every `seek` a connection gets a full `state`: position, length, pending seats, drivers, one table model per observer, prompts, the move log, the outcome. Table models and choices use game-core's serde JSON as documented in [design/game-core.md](../design/game-core.md).
- **Observers are explicit.** `create` and `join` list the views a connection wants (`{"seat": n}`, `"spectator"`, `"omniscient"`); default is every seat plus spectator (plus omniscient in a dev build).
- **Dev-only data.** Only a build with the server's `dev` feature accepts `"omniscient"` and sends the master seed, the full game record (it contains the seed, which determines every hidden card) and per-move game events (not yet filtered per observer). A build without it rejects `"omniscient"` and leaves those fields and messages out. The gate is the server crate's own feature, not the existence of `Observer::Omniscient`, which Cargo feature unification can enable through other crates.
- **Log stepping is per connection.** `seek` with a position shows the match after that many moves, rebuilt from the record (replay is deterministic); `seek` without one follows the live match. Prompts are sent only while live.

Messages in v1:

| Direction | Type | Purpose |
| --- | --- | --- |
| client → server | `hello` | Pick the locale; answered by `welcome` |
| | `create` | Start a match (game, seats, seed, variant, drivers, observers) and join it |
| | `join` | Join a running match by number |
| | `choose`, `undo`, `submit` | Selection protocol for one human seat |
| | `set-driver` | Switch a seat between `human` and `bot` |
| | `seek` | Move this connection's position in the log, or follow live |
| server → client | `welcome` | Protocol, build, dev flag, locales, games with default variants, messages |
| | `joined` | Match number, game, seats, observers (seed in dev builds) |
| | `state` | The match as this connection sees it |
| | `events` | A move's game events (dev builds only) |
| | `error` | `code` (message ID), `args`, optional game `reason` ID |

The exact fields are in [design/server.md](../design/server.md) and in `crates/server/src/protocol.rs`.

## Consequences

- Any client that speaks JSON over WebSocket can drive matches without game logic; the dev page is plain JavaScript.
- Full states are simple and robust but grow with long logs; deltas or log pagination can come in a later version.
- Every change to message shapes needs a new protocol version and an update of this ADR (or a superseding one).
- `crates/server/tests/protocol.rs` pins the JSON of every message kind.

## Open points

- Authentication and seat ownership: in v1 any connection may drive and watch any seat of a match. The public server needs seat claims, reconnection tokens and per-seat views only.
- Per-observer events, so `events` can leave the dev build.
- Binary encoding (ADR-0003) as a second encoding, negotiated in `hello`.
- Client build check and compatibility window ([ADR-0004](0004-update-policy.md)).
- Whether clients should format plural messages themselves (shipping Fluent to the client) instead of avoiding selectors.
- Deltas instead of full states, presence and spectator delay.
