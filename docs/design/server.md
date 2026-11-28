# server

The `libretable-server` crate (AGPL-3.0-or-later, see [policies](../policies.md#licences)) builds the binary `table-server`: a WebSocket server that hosts matches through [`Session`](session.md) and serves the web dev multiclient. It is the seed of the real server ([architecture: server](../architecture.md#server)). The wire format is protocol v1, below.

## Running it

```text
cargo run -p libretable-server --features dev     # dev build: omniscient view, records, events
cargo run -p libretable-server                    # without them
cargo run -p libretable-server -- --listen 127.0.0.1:9000
```

Then open <http://127.0.0.1:8080/>. The server listens on `127.0.0.1:8080` by default; it has no authentication, so keep it on loopback. Console messages use the locale of `LC_ALL`, `LC_MESSAGES` or `LANG`.

The page: pick a game, the number of seats, an optional seed and the variant (JSON, prefilled with the standard ruleset) and press *New match*. Every seat gets a column with its table, who plays it (human or bot, switchable at any time) and, when it must act, its prompt: the choices made, the selectable ones (also clickable in the table) and *Undo* / *Submit*. The spectator view and, in a dev build, the omniscient view sit beside them. The move log below lists every move (its choices if you watch the moving seat); *First*, *Back*, *Forward*, *Last*, the slider or a click on a log entry show the match at that point, *Live* returns. In a dev build the game record can be downloaded. The URL carries the match number (`#match=3`), so a second tab or browser can open the same match.

## Crate layout

| Module | Purpose |
| --- | --- |
| `protocol` | Wire types (public): `ClientMessage`, `ServerMessage`, `encode`, `decode`. |
| `connection` | `Server` (all matches, kept until the process ends) and `Connection` (one client: its match, observers and log position). No networking, so tests drive it directly. |
| `hosted` | One match with its game type erased: the session, seat drivers, each human seat's draft, and per seat one `RandomBot` and, for searchable games, one `IsmctsBot` (300 iterations). |
| `web` | axum routes: `/` (page), `/app.js`, `/style.css`, `/ws`. |
| `main.rs` | Command line and start-up. |

Games come from `libretable-registry`, shared with the CLI: a `Visitor` gets the game's type for an ID, and `GAMES` lists the IDs with their message IDs.

The page lives in `apps/web/dev/` (plain HTML, CSS and JavaScript, no build step) and is embedded into the binary at compile time with `include_str!`, so the binary is self-contained.

## Protocol v1

Every frame is a JSON object `{"v": 1, "type": "…", …}`. Field names are kebab-case. Seats are numbers from 0. Choices, table models and seat results use game-core's JSON ([game-core.md](game-core.md)): e.g. `{"option": "rps-rock"}`, `{"component": 3}`, `{"subset": [2, 4]}`.

### Client → server

| Type | Fields | Effect |
| --- | --- | --- |
| `hello` | `locale?` | Answered by `welcome`; may be sent again to switch language. |
| `create` | `game`, `seats`, `seed?`, `variant?`, `drivers?`, `observers?` | Starts a match and joins it: `joined`, then `state`. `drivers` default to all `"human"`; `observers` default to every seat, `"spectator"` and (dev) `"omniscient"`. Without `seed` the server picks one below 2^53. |
| `join` | `match`, `observers?` | Joins a running match. A connection is in at most one match. |
| `choose` | `seat`, `choice` | Adds a choice to the seat's draft; it must be selectable. |
| `undo` | `seat` | Drops the draft's last choice. |
| `submit` | `seat` | Applies the draft once complete; bots then act. |
| `set-driver` | `seat`, `driver` | `"human"`, `"bot"` (random) or `"ismcts"` (games with `ismcts` in `welcome.games` only, else `server-error-no-ismcts`). A bot seat that must act plays at once; its draft is dropped. |
| `seek` | `position?` | Shows the match after `position` moves (`0` … `length`); without it, follows live. |

### Server → client

| Type | Fields |
| --- | --- |
| `welcome` | `protocol`, `build`, `dev`, `locale`, `locales`, `games` (`id`, default `variant`, `ismcts`: whether the ISMCTS bot can drive seats), `messages` |
| `joined` | `match`, `game`, `seats`, `observers`, `seed` (dev only) |
| `state` | `match`, `position`, `length`, `live`, `pending` (`kind`: `all` / `any` / `finished`, `seats`), `drivers`, `views` (`observer`, `table`), `prompts` (`seat`, `made`, `selectable`, `complete`), `log` (`seat`, `choices` if observed), `outcome?` (`seats`: `rank`, `score`; `teams`), `record` (dev only) |
| `events` | `match`, `position`, `seat`, `events` (the game's events as JSON; dev only) |
| `error` | `code`, `args?`, `reason?` |

- Every change to a match (a choice, an undo, a driver switch, moves) sends a fresh `state` to every connection in it; with moves, a dev build first sends one `events` per move. Requests that fail answer only the sender with `error` and change nothing.
- `prompts` are sent only while `live`, for observed seats that must act and are driven by a human. While a connection looks at the past, choices still go to the live match, but the page disables input.
- The log is the whole match whatever the position; `choices` appear only for moves of seats the connection observes (or all, with `"omniscient"`), described against the moving seat's view at that time.
- Bots: one `RandomBot` per seat, seeded like the CLI from the master seed (`session::derive_seed`), so a bot match with a given seed always plays out the same. Bots act right after the request that made them pending, until a human must act or the match ends.
- Matches are rebuilt from the record when a connection seeks into the past; nothing but the live session is stored.

### Errors

`code` is a message ID from `locales/*/server.ftl`: `server-error-protocol-version`, `-bad-message`, `-unknown-game`, `-bad-seats`, `-drivers`, `-variant-json`, `-invalid-variant`, `-unknown-match`, `-no-match`, `-bad-seat`, `-seat-is-bot`, `-no-ismcts`, `-not-pending`, `-not-selectable`, `-incomplete`, `-ambiguous`, `-illegal`, `-bad-position`, `-omniscient-unavailable`, `-internal`. `args` are its arguments as text. `reason`, when present, is a game's message ID (an invalid variant or illegal move); the client formats it and passes it as the `reason` argument.

## The `dev` feature

`--features dev` enables `libretable-game-core/dev` and, in the server, `"omniscient"` observers, `joined.seed`, `state.record` and `events`. Without it, `"omniscient"` is rejected with `server-error-omniscient-unavailable` and the other fields and messages are never sent. The gate is the server's own `cfg(feature = "dev")` in one function (`connection::observer`), not the existence of `Observer::Omniscient`: in a workspace build Cargo enables game-core's `dev` feature through `conformance`'s dev-dependency. `tests/dev_gate.rs` checks both builds; CI runs the server's clippy and tests with and without the feature.

## Localization

All page text and every error are message IDs in `locales/*/server.ftl` (`server-…`), listed in `libretable_server::MESSAGE_IDS`. `welcome.messages` carries these plus every registered game's `MESSAGE_IDS`, formatted for the requested locale (negotiated like the CLI's `--lang`) with arguments left as `{$name}`; the page fills them in. Hence page messages use no plural selectors. Face-up components are named by their face key, a message ID ([localization](localization.md)); card names come with Mau-Mau's `MESSAGE_IDS`. Tests: every listed ID has an `en` message, every `server-` message is listed, and every `server-…` ID in `apps/web/dev/` is listed (`tests/messages.rs`).

## Tests

- `tests/protocol.rs`: exact JSON and round trip of every message kind; version check before parsing.
- `tests/connection.rs`: whole matches by hand and by bot, undo and rejections, driver switch, log stepping, a second connection watching one seat.
- `tests/dev_gate.rs`: with and without `dev`.
- `tests/websocket.rs`: a real server on `127.0.0.1:0`: page and assets over HTTP, a whole rock-paper-scissors match over `/ws` with two clients, the omniscient gate. Binding a port can be refused inside a sandbox (EPERM); run it outside then.

Not yet: reconnection and persistence; see [open points](#open-points).

## Design notes

- **Dev server first, Wasm later.** The dev server is the seed of the real server and exercises the WebSocket protocol from the first milestone; Wasm comes in the web stage for offline play and on-device bots.
- **Plain HTML, CSS and JavaScript, no framework and no build step.** The page only renders what the server sends and forwards choices, so a framework would add tooling without removing much code; no Node.js toolchain in the repository or CI. The public web client decides its own tooling.
- **Text is never prose** on the wire: labels, options, reasons and error codes are message IDs, formatted by the server for the client's locale.
- **State, not deltas:** simple and robust, but grows with long logs.
- **Versioning:** every change to message shapes bumps the protocol version and updates this document; `tests/protocol.rs` pins the JSON of every message kind.

## Open points

- Authentication and seat ownership: in v1 any connection may drive and watch any seat. The public server needs seat claims, reconnection tokens and per-seat views only.
- Per-observer events, so `events` can leave the dev build.
- The binary encoding ([game-core.md](game-core.md#encoding)) as a second encoding, negotiated in `hello`.
- Client build check and compatibility window ([architecture: saves, replays & updates](../architecture.md#saves-replays--updates)).
- Whether clients should format plural messages themselves (shipping Fluent to the client) instead of avoiding selectors.
- Deltas instead of full states, presence and spectator delay.
