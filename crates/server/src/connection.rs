//! The server's matches and what one client connection does with them,
//! without any networking: [`Connection`] turns decoded client messages into
//! server messages, so tests drive it directly and the WebSocket layer
//! (`web`) only moves text frames.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use libretable_game_core::{Observer, Seat, SeatCount};
use libretable_l10n::{LOCALES, Localizer};
use libretable_registry::GAMES;
use tokio::sync::broadcast;

use crate::error::ServerError;
use crate::hosted::{Create, DefaultVariant, Hosted, Moved};
use crate::protocol::{
    ClientMessage, DecodeError, Driver, ErrorMessage, GameInfo, MatchState, PROTOCOL_VERSION,
    ServerMessage, WireObserver, decode,
};
use crate::{DEV, MESSAGE_IDS};

/// Changes kept for a connection that falls behind; it then gets a fresh
/// `state` instead of the missed ones.
const CHANGE_BUFFER: usize = 64;

/// All matches of one server process. Matches live until the process ends.
#[derive(Debug)]
pub struct Server {
    build: &'static str,
    rooms: Mutex<HashMap<u64, Arc<Room>>>,
    next_id: AtomicU64,
}

/// One match and the connections watching it.
pub struct Room {
    id: u64,
    game: &'static str,
    seats: SeatCount,
    seed: u64,
    hosted: Mutex<Box<dyn Hosted>>,
    changes: broadcast::Sender<Change>,
}

impl std::fmt::Debug for Room {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Room")
            .field("id", &self.id)
            .field("game", &self.game)
            .finish_non_exhaustive()
    }
}

/// Something changed in a match: moves were applied (possibly none, when
/// only a selection or a driver changed).
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    /// The moves applied, in order.
    pub moves: Vec<Moved>,
}

impl Server {
    /// A server without matches. `build` goes into records and `welcome`.
    #[must_use]
    pub fn new(build: &'static str) -> Arc<Self> {
        Arc::new(Self {
            build,
            rooms: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        })
    }

    fn room(&self, id: u64) -> Option<Arc<Room>> {
        lock(&self.rooms).get(&id).cloned()
    }
}

impl Room {
    fn hosted(&self) -> MutexGuard<'_, Box<dyn Hosted>> {
        lock(&self.hosted)
    }
}

/// A poisoned lock only means another connection panicked mid-request; the
/// match data is still consistent because every change is all-or-nothing.
fn lock<T: ?Sized>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One client: at most one joined match, the views it receives and its
/// position in the move log.
#[derive(Debug)]
pub struct Connection {
    server: Arc<Server>,
    joined: Option<Joined>,
}

#[derive(Debug)]
struct Joined {
    room: Arc<Room>,
    observers: Vec<(WireObserver, Observer)>,
    /// `None` follows the live match.
    cursor: Option<usize>,
}

impl Connection {
    /// A new connection to `server`.
    #[must_use]
    pub fn new(server: Arc<Server>) -> Self {
        Self {
            server,
            joined: None,
        }
    }

    /// The joined match's ID.
    #[must_use]
    pub fn match_id(&self) -> Option<u64> {
        self.joined.as_ref().map(|joined| joined.room.id)
    }

    /// Changes of the joined match. Subscribe again after the match ID
    /// changed.
    #[must_use]
    pub fn changes(&self) -> Option<broadcast::Receiver<Change>> {
        self.joined
            .as_ref()
            .map(|joined| joined.room.changes.subscribe())
    }

    /// Handles one text frame.
    pub fn receive(&mut self, text: &str) -> Vec<ServerMessage> {
        match decode::<ClientMessage>(text) {
            Ok(message) => self.handle(message),
            Err(DecodeError::Version(version)) => vec![error(
                ServerError::new("server-error-protocol-version")
                    .arg("expected", PROTOCOL_VERSION)
                    .arg("version", version),
            )],
            Err(DecodeError::Malformed(detail)) => vec![error(
                ServerError::new("server-error-bad-message").arg("detail", detail),
            )],
        }
    }

    /// Handles one message. Replies go to this connection; changes to the
    /// match go to every connection in it through [`changes`](Self::changes),
    /// this one included.
    pub fn handle(&mut self, message: ClientMessage) -> Vec<ServerMessage> {
        let result = match message {
            ClientMessage::Hello { locale } => Ok(vec![welcome(
                self.server.build,
                locale.as_deref().unwrap_or("en"),
            )]),
            ClientMessage::Create {
                game,
                seats,
                seed,
                variant,
                drivers,
                observers,
            } => self.create(&game, seats, seed, variant.as_ref(), drivers, observers),
            ClientMessage::Join {
                match_id,
                observers,
            } => self.join(match_id, observers),
            ClientMessage::Choose { seat, choice } => {
                self.change(|hosted| hosted.choose(seat, choice).map(|()| Vec::new()))
            }
            ClientMessage::Undo { seat } => {
                self.change(|hosted| hosted.undo(seat).map(|()| Vec::new()))
            }
            ClientMessage::Submit { seat } => self.change(|hosted| hosted.submit(seat)),
            ClientMessage::SetDriver { seat, driver } => {
                self.change(|hosted| hosted.set_driver(seat, driver))
            }
            ClientMessage::Seek { position } => self.seek(position),
        };
        result.unwrap_or_else(|failure| vec![error(failure)])
    }

    /// What to send after `change` of the joined match.
    #[must_use]
    pub fn on_change(&self, change: &Change) -> Vec<ServerMessage> {
        let mut messages = Vec::new();
        if DEV {
            if let Some(joined) = &self.joined {
                messages.extend(change.moves.iter().map(|moved| ServerMessage::Events {
                    match_id: joined.room.id,
                    position: moved.position,
                    seat: moved.seat,
                    events: moved.events.clone(),
                }));
            }
        }
        messages.extend(self.state());
        messages
    }

    /// What to send after missing changes: the current state.
    #[must_use]
    pub fn on_lagged(&self) -> Vec<ServerMessage> {
        self.state().into_iter().collect()
    }

    fn create(
        &mut self,
        game: &str,
        seats: u8,
        seed: Option<u64>,
        variant: Option<&serde_json::Value>,
        drivers: Option<Vec<Driver>>,
        observers: Option<Vec<WireObserver>>,
    ) -> Result<Vec<ServerMessage>, ServerError> {
        let entry = libretable_registry::find(game)
            .ok_or_else(|| ServerError::new("server-error-unknown-game").arg("game", game))?;
        let count = SeatCount::new(seats)
            .ok_or_else(|| ServerError::new("server-error-bad-seats").arg("seats", seats))?;
        let observers = resolve(observers, count)?;
        let seed = seed.unwrap_or_else(clock_seed);
        let hosted = libretable_registry::visit(
            entry.id,
            Create {
                seats: count,
                seed,
                variant,
                drivers: drivers.unwrap_or_else(|| vec![Driver::Human; usize::from(seats)]),
                build: self.server.build,
            },
        )
        .ok_or_else(|| ServerError::new("server-error-unknown-game").arg("game", game))??;
        let id = self.server.next_id.fetch_add(1, Ordering::Relaxed);
        let room = Arc::new(Room {
            id,
            game: entry.id,
            seats: count,
            seed,
            hosted: Mutex::new(hosted),
            changes: broadcast::channel(CHANGE_BUFFER).0,
        });
        lock(&self.server.rooms).insert(id, Arc::clone(&room));
        Ok(self.enter(room, observers))
    }

    fn join(
        &mut self,
        id: u64,
        observers: Option<Vec<WireObserver>>,
    ) -> Result<Vec<ServerMessage>, ServerError> {
        let room = self
            .server
            .room(id)
            .ok_or_else(|| ServerError::new("server-error-unknown-match").arg("match", id))?;
        let observers = resolve(observers, room.seats)?;
        Ok(self.enter(room, observers))
    }

    fn enter(
        &mut self,
        room: Arc<Room>,
        observers: Vec<(WireObserver, Observer)>,
    ) -> Vec<ServerMessage> {
        let joined = ServerMessage::Joined {
            match_id: room.id,
            game: room.game.to_owned(),
            seats: room.seats.get(),
            observers: observers.iter().map(|(wire, _)| *wire).collect(),
            seed: DEV.then_some(room.seed),
        };
        self.joined = Some(Joined {
            room,
            observers,
            cursor: None,
        });
        let mut messages = vec![joined];
        messages.extend(self.state());
        messages
    }

    fn change(
        &mut self,
        apply: impl FnOnce(&mut dyn Hosted) -> Result<Vec<Moved>, ServerError>,
    ) -> Result<Vec<ServerMessage>, ServerError> {
        let joined = self
            .joined
            .as_ref()
            .ok_or_else(|| ServerError::new("server-error-no-match"))?;
        let moves = apply(joined.room.hosted().as_mut())?;
        // No receivers is fine: then nobody needs to hear about it.
        let _ = joined.room.changes.send(Change { moves });
        Ok(Vec::new())
    }

    fn seek(&mut self, position: Option<usize>) -> Result<Vec<ServerMessage>, ServerError> {
        let joined = self
            .joined
            .as_mut()
            .ok_or_else(|| ServerError::new("server-error-no-match"))?;
        if let Some(position) = position {
            let length = joined.room.hosted().len();
            if position > length {
                return Err(ServerError::new("server-error-bad-position")
                    .arg("position", position)
                    .arg("length", length));
            }
        }
        joined.cursor = position;
        Ok(self.state().into_iter().collect())
    }

    /// The `state` message for this connection, if it is in a match.
    fn state(&self) -> Option<ServerMessage> {
        let joined = self.joined.as_ref()?;
        let hosted = joined.room.hosted();
        let message = match hosted.snapshot(&joined.observers, joined.cursor) {
            Ok(snapshot) => ServerMessage::State(MatchState {
                match_id: joined.room.id,
                position: snapshot.position,
                length: snapshot.length,
                live: joined.cursor.is_none(),
                pending: snapshot.pending,
                drivers: snapshot.drivers,
                views: snapshot.views,
                prompts: snapshot.prompts,
                log: snapshot.log,
                outcome: snapshot.outcome,
                record: DEV.then(|| hosted.record()),
            }),
            Err(failure) => error(failure),
        };
        Some(message)
    }
}

fn error(error: ServerError) -> ServerMessage {
    ServerMessage::Error(ErrorMessage::from(error))
}

/// The `welcome` message: every message the server or a game can send,
/// formatted for `locale` with arguments left as `{$name}`.
fn welcome(build: &str, locale: &str) -> ServerMessage {
    let l10n = Localizer::new(locale);
    let ids = MESSAGE_IDS
        .iter()
        .chain(GAMES.iter().flat_map(|entry| entry.message_ids.iter()));
    let messages: BTreeMap<String, String> = ids
        .map(|id| ((*id).to_owned(), l10n.format(id, &[])))
        .collect();
    ServerMessage::Welcome {
        protocol: PROTOCOL_VERSION,
        build: build.to_owned(),
        dev: DEV,
        locale: l10n.locale().to_owned(),
        locales: LOCALES.iter().map(|locale| (*locale).to_owned()).collect(),
        games: GAMES
            .iter()
            .map(|entry| GameInfo {
                id: entry.id.to_owned(),
                variant: libretable_registry::visit(entry.id, DefaultVariant)
                    .unwrap_or(serde_json::Value::Null),
                ismcts: entry.searchable,
            })
            .collect(),
        messages,
    }
}

/// The requested observers, checked; by default every seat, the spectator
/// and, in a dev build, the omniscient view.
fn resolve(
    requested: Option<Vec<WireObserver>>,
    seats: SeatCount,
) -> Result<Vec<(WireObserver, Observer)>, ServerError> {
    let requested = requested.unwrap_or_else(|| {
        let mut all: Vec<WireObserver> = seats
            .seats()
            .map(|seat| WireObserver::Seat(seat.index()))
            .collect();
        all.push(WireObserver::Spectator);
        if DEV {
            all.push(WireObserver::Omniscient);
        }
        all
    });
    requested
        .into_iter()
        .map(|wire| observer(wire, seats).map(|observer| (wire, observer)))
        .collect()
}

/// The core observer for a wire observer. The only place that can produce
/// `Observer::Omniscient`, and only with this crate's `dev` feature: Cargo
/// may enable game-core's `dev` feature through another crate, so the gate is
/// this crate's own feature, not the variant's existence.
fn observer(wire: WireObserver, seats: SeatCount) -> Result<Observer, ServerError> {
    match wire {
        WireObserver::Seat(index) => Seat::new(index)
            .filter(|seat| seats.contains(*seat))
            .map(Observer::Seat)
            .ok_or_else(|| ServerError::new("server-error-bad-seat").arg("seat", index)),
        WireObserver::Spectator => Ok(Observer::Spectator),
        WireObserver::Omniscient => omniscient(),
    }
}

#[cfg(feature = "dev")]
fn omniscient() -> Result<Observer, ServerError> {
    Ok(Observer::Omniscient)
}

#[cfg(not(feature = "dev"))]
fn omniscient() -> Result<Observer, ServerError> {
    Err(ServerError::new("server-error-omniscient-unavailable"))
}

/// A seed for `create` without one.
fn clock_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            // The low 64 bits of the nanoseconds are plenty for a seed; keep
            // it below 2^53 so JavaScript clients can show it exactly.
            #[allow(clippy::cast_possible_truncation)]
            let nanos = elapsed.as_nanos() as u64;
            nanos & ((1 << 53) - 1)
        })
}
