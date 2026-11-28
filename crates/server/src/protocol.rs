//! The WebSocket wire format, version [`PROTOCOL_VERSION`] (docs/design/server.md).
//!
//! Every message is one JSON text frame: an object with the protocol version
//! `v`, a kebab-case `type` and the message's fields. Clients send
//! [`ClientMessage`]s, the server answers with [`ServerMessage`]s. Text is
//! never sent as prose: labels, options, reasons and error codes are Fluent
//! message IDs, which the client formats with the `messages` of
//! [`ServerMessage::Welcome`].
//!
//! The types do not depend on the `dev` feature, so both builds speak the
//! same format; a build without it rejects [`WireObserver::Omniscient`] and
//! leaves the dev-only fields out.

use std::collections::BTreeMap;

use libretable_game_core::{Choice, SeatResult, TableModel};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The protocol version in every message's `v`. A message with another
/// version is rejected with `server-error-protocol-version`.
pub const PROTOCOL_VERSION: u32 = 1;

/// A message with its protocol version: `{"v": 1, "type": "...", ...}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Envelope<T> {
    /// Protocol version.
    pub v: u32,
    /// The message; its fields sit next to `v`.
    #[serde(flatten)]
    pub message: T,
}

/// Who a seat is driven by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Driver {
    /// Choices arrive from clients through the selection protocol.
    Human,
    /// The server plays the seat with a seeded random bot.
    Bot,
}

/// Whose view a client wants: `{"seat": 0}`, `"spectator"` or
/// `"omniscient"` (dev builds only).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WireObserver {
    /// One seat's view, numbered from 0.
    Seat(u8),
    /// Public information only.
    Spectator,
    /// The full state. Refused by a build without the `dev` feature.
    Omniscient,
}

/// Client → server.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case"
)]
pub enum ClientMessage {
    /// Opens the conversation and picks the locale of `welcome.messages`.
    /// May be sent again to change the locale.
    Hello {
        /// A BCP 47 tag such as `"de-DE"`; unsupported tags get English.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        locale: Option<String>,
    },
    /// Starts a match and joins it.
    Create {
        /// Game ID, one of `welcome.games`.
        game: String,
        /// Number of seats.
        seats: u8,
        /// Master seed; the match seed and the bot seeds derive from it.
        /// Default: chosen by the server. JavaScript clients should stay
        /// below 2^53.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seed: Option<u64>,
        /// The game's config as JSON; default: the standard ruleset.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        variant: Option<Value>,
        /// One driver per seat; default: all human.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        drivers: Option<Vec<Driver>>,
        /// Views to receive; default: every seat, the spectator and, in a
        /// dev build, the omniscient view.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        observers: Option<Vec<WireObserver>>,
    },
    /// Joins a running match. A connection is in at most one match; joining
    /// another leaves the first.
    Join {
        /// The match ID from `joined`.
        #[serde(rename = "match")]
        match_id: u64,
        /// As for `create`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        observers: Option<Vec<WireObserver>>,
    },
    /// Adds one choice to a human seat's selection.
    Choose {
        /// The choosing seat.
        seat: u8,
        /// One of the seat's `prompts[].selectable`.
        choice: Choice,
    },
    /// Drops the seat's last choice; nothing happens if it has none.
    Undo {
        /// The seat.
        seat: u8,
    },
    /// Submits the seat's choices once its prompt is `complete`.
    Submit {
        /// The seat.
        seat: u8,
    },
    /// Switches who drives a seat. A bot takes over at once if the seat
    /// must act.
    SetDriver {
        /// The seat.
        seat: u8,
        /// The new driver.
        driver: Driver,
    },
    /// Moves this connection's position in the move log: `null` (or no
    /// field) follows the live match, a number shows the match after that
    /// many moves.
    Seek {
        /// Moves applied in what is shown; at most `state.length`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        position: Option<usize>,
    },
}

/// Server → client.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case"
)]
pub enum ServerMessage {
    /// Answer to `hello`.
    Welcome {
        /// [`PROTOCOL_VERSION`].
        protocol: u32,
        /// The server's build ID.
        build: String,
        /// Whether this is a dev build (omniscient view, records, events).
        dev: bool,
        /// The locale chosen for `messages`, e.g. `"de"`.
        locale: String,
        /// Every supported locale.
        locales: Vec<String>,
        /// The games that can be created.
        games: Vec<GameInfo>,
        /// Message ID → text in `locale`, for every ID the server or a game
        /// can send. Arguments are left as `{$name}` for the client to fill.
        messages: BTreeMap<String, String>,
    },
    /// The connection is now in a match; a `state` follows.
    Joined {
        /// The match ID, for `join` from other connections.
        #[serde(rename = "match")]
        match_id: u64,
        /// Game ID.
        game: String,
        /// Number of seats.
        seats: u8,
        /// The views this connection receives, in the order of
        /// `state.views`.
        observers: Vec<WireObserver>,
        /// The master seed; dev builds only, since it reveals every hidden
        /// card.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seed: Option<u64>,
    },
    /// The match as this connection sees it, after every change and every
    /// `seek`.
    State(MatchState),
    /// What one move caused, as the game's own JSON events. Dev builds
    /// only: events are not yet filtered per observer.
    Events {
        /// The match ID.
        #[serde(rename = "match")]
        match_id: u64,
        /// Moves applied after this one, i.e. its 1-based number.
        position: usize,
        /// The seat that moved.
        seat: u8,
        /// The game's events of the move and the chance steps after it.
        events: Vec<Value>,
    },
    /// A request was rejected; nothing changed.
    Error(ErrorMessage),
}

/// One game in `welcome`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GameInfo {
    /// Game ID; also the message ID of its display name.
    pub id: String,
    /// The standard ruleset as JSON, a starting point for `create.variant`.
    pub variant: Value,
}

/// The `state` message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MatchState {
    /// The match ID.
    #[serde(rename = "match")]
    pub match_id: u64,
    /// Moves applied in what is shown.
    pub position: usize,
    /// Moves in the match so far.
    pub length: usize,
    /// Whether `position` follows the live match (`seek` with `null`).
    /// Prompts are sent only then.
    pub live: bool,
    /// Who must act at `position`.
    pub pending: WirePending,
    /// The driver of every seat, live.
    pub drivers: Vec<Driver>,
    /// One table model per observer of `joined.observers`, at `position`.
    pub views: Vec<ObserverView>,
    /// The selection of every observed human seat that must act, live.
    pub prompts: Vec<WirePrompt>,
    /// Every move of the match, in order.
    pub log: Vec<LogEntry>,
    /// The result at `position`, once finished.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<WireOutcome>,
    /// The whole game record (with its seed); dev builds only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<Value>,
}

/// Who must act.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct WirePending {
    /// `all` (every listed seat once), `any` (any listed seat) or
    /// `finished`.
    pub kind: PendingKind,
    /// The listed seats, ascending.
    pub seats: Vec<u8>,
}

/// The kind of [`WirePending`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PendingKind {
    /// Every listed seat submits once (simultaneous decisions).
    All,
    /// Any listed seat may act.
    Any,
    /// The match is over.
    Finished,
}

/// One observer's table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ObserverView {
    /// Whose view.
    pub observer: WireObserver,
    /// The table model of that view (docs/design/game-core.md).
    pub table: TableModel,
}

/// A seat's selection: what it chose so far and may choose next.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct WirePrompt {
    /// The seat.
    pub seat: u8,
    /// Choices made so far.
    pub made: Vec<Choice>,
    /// The next selectable choices; empty once `complete`.
    pub selectable: Vec<Choice>,
    /// Whether `made` is a whole move, so `submit` is accepted.
    pub complete: bool,
}

/// One move of the log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LogEntry {
    /// The seat that moved.
    pub seat: u8,
    /// The move's choice path as the moving seat saw it, if this connection
    /// observes that seat (or everything); otherwise absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<Choice>>,
}

/// The result of a finished match.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct WireOutcome {
    /// Per seat, in seat order: rank (1 = best, ties share) and score.
    pub seats: Vec<SeatResult>,
    /// Partnerships as lists of seats; empty without teams.
    pub teams: Vec<Vec<u8>>,
}

/// The `error` message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ErrorMessage {
    /// What went wrong, as a message ID (`server-error-…`).
    pub code: String,
    /// Arguments of the message, as text.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, String>,
    /// A game's message ID explaining the rejection (an invalid variant or
    /// illegal move); the client formats it and passes it as the `reason`
    /// argument.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Why a client message could not be decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// Not a JSON object with a numeric `v`, or not a known message.
    Malformed(String),
    /// `v` is not [`PROTOCOL_VERSION`].
    Version(u64),
}

/// Encodes `message` as one text frame.
#[must_use]
pub fn encode<T: Serialize>(message: &T) -> String {
    let envelope = Envelope {
        v: PROTOCOL_VERSION,
        message,
    };
    // Our message types contain only maps with string keys and plain data,
    // which serde_json always serializes.
    serde_json::to_string(&envelope).unwrap_or_default()
}

/// Decodes one text frame. The version is checked first, so a message of a
/// newer protocol is reported as [`DecodeError::Version`], not as malformed.
///
/// # Errors
///
/// [`DecodeError`].
pub fn decode<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, DecodeError> {
    let value: Value =
        serde_json::from_str(text).map_err(|error| DecodeError::Malformed(error.to_string()))?;
    let version = value
        .get("v")
        .and_then(Value::as_u64)
        .ok_or_else(|| DecodeError::Malformed("missing protocol version `v`".to_owned()))?;
    if version != u64::from(PROTOCOL_VERSION) {
        return Err(DecodeError::Version(version));
    }
    serde_json::from_value::<Envelope<T>>(value)
        .map(|envelope| envelope.message)
        .map_err(|error| DecodeError::Malformed(error.to_string()))
}
