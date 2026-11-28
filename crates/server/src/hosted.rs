//! One hosted match with its type erased: the session plus what the server
//! keeps around it (seat drivers, each human seat's choices so far, bots).
//!
//! The session itself stays stateless about selection (docs/design/
//! session.md); the drafts live here, in the transport layer, so every
//! connection watching a seat sees the same selection.

use libretable_bots::{IsmctsBot, IsmctsConfig, Player, RandomBot};
use libretable_game_core::{
    Choice, Determinize, Game, GameRng, Observer, Pending, RecordOf, Seat, SeatCount, Seed,
};
use libretable_registry::{RegisteredGame, Visitor};
use libretable_session::{Session, SubmitError, derive_seed};
use serde_json::Value;

use crate::error::ServerError;
use crate::protocol::{
    Driver, LogEntry, ObserverView, PendingKind, WireObserver, WireOutcome, WirePending, WirePrompt,
};

/// Upper bound on bot moves in one go, so a game that never ends cannot hang
/// the server; the rest waits for the next request.
const MAX_BOT_MOVES: usize = 100_000;

/// One applied move, for the `events` message.
#[derive(Clone, Debug, PartialEq)]
pub struct Moved {
    /// The seat that moved.
    pub seat: u8,
    /// Moves in the record after this one.
    pub position: usize,
    /// The game's events, as JSON.
    pub events: Vec<Value>,
}

/// What a hosted match shows one connection: [`crate::protocol::MatchState`]
/// without the fields the connection adds.
pub struct Snapshot {
    pub position: usize,
    pub length: usize,
    pub pending: WirePending,
    pub drivers: Vec<Driver>,
    pub views: Vec<ObserverView>,
    pub prompts: Vec<WirePrompt>,
    pub log: Vec<LogEntry>,
    pub outcome: Option<WireOutcome>,
}

/// A match of some game.
pub trait Hosted: Send {
    /// Moves applied so far.
    fn len(&self) -> usize;

    /// The match for `observers` after `position` moves (`None`: live).
    fn snapshot(
        &self,
        observers: &[(WireObserver, Observer)],
        position: Option<usize>,
    ) -> Result<Snapshot, ServerError>;

    /// Adds `choice` to the seat's draft.
    fn choose(&mut self, seat: u8, choice: Choice) -> Result<(), ServerError>;

    /// Drops the last choice of the seat's draft.
    fn undo(&mut self, seat: u8) -> Result<(), ServerError>;

    /// Submits the seat's draft, then lets bots act.
    fn submit(&mut self, seat: u8) -> Result<Vec<Moved>, ServerError>;

    /// Changes a seat's driver, then lets bots act.
    fn set_driver(&mut self, seat: u8, driver: Driver) -> Result<Vec<Moved>, ServerError>;

    /// The game record as JSON.
    fn record(&self) -> Value;
}

/// Creates a hosted match of the visited game.
pub struct Create<'a> {
    pub seats: SeatCount,
    pub seed: u64,
    pub variant: Option<&'a Value>,
    pub drivers: Vec<Driver>,
    pub build: &'static str,
}

impl Visitor for Create<'_> {
    type Output = Result<Box<dyn Hosted>, ServerError>;

    fn visit<G: RegisteredGame>(self) -> Self::Output {
        Match::<G>::new(self, None).map(|hosted| Box::new(hosted) as Box<dyn Hosted>)
    }

    fn visit_searchable<G: RegisteredGame + Determinize>(self) -> Self::Output {
        Match::<G>::new(self, Some(ismcts::<G>)).map(|hosted| Box::new(hosted) as Box<dyn Hosted>)
    }
}

/// ISMCTS iterations per decision on the server; kept small so a debug
/// build answers quickly.
pub const ISMCTS_ITERATIONS: u32 = 300;

/// A seat's search bot.
type SearchBot<G> = Box<dyn Player<G> + Send>;

fn ismcts<G: Determinize>(seed: Seed) -> SearchBot<G> {
    Box::new(IsmctsBot::with_config(
        seed,
        IsmctsConfig {
            iterations: ISMCTS_ITERATIONS,
            ..IsmctsConfig::default()
        },
    ))
}

/// The standard ruleset of the visited game, as JSON.
pub struct DefaultVariant;

impl Visitor for DefaultVariant {
    type Output = Value;

    fn visit<G: RegisteredGame>(self) -> Value {
        serde_json::to_value(G::Config::default()).unwrap_or(Value::Null)
    }
}

struct Match<G: Game> {
    session: Session<G>,
    drivers: Vec<Driver>,
    /// Per seat: the choices made so far.
    drafts: Vec<Vec<Choice>>,
    /// Per seat, used while the seat is driven by the random bot.
    bots: Vec<RandomBot>,
    /// Per seat, used while the seat is driven by the ISMCTS bot; empty for
    /// games without [`Determinize`].
    search: Vec<SearchBot<G>>,
}

impl<G: RegisteredGame> Match<G> {
    fn new(
        create: Create<'_>,
        search: Option<fn(Seed) -> SearchBot<G>>,
    ) -> Result<Self, ServerError> {
        let config: G::Config = match create.variant {
            None => G::Config::default(),
            Some(json) => serde_json::from_value(json.clone()).map_err(|error| {
                ServerError::new("server-error-variant-json").arg("detail", error)
            })?,
        };
        let count = usize::from(create.seats.get());
        if create.drivers.len() != count {
            return Err(ServerError::new("server-error-drivers")
                .arg("seats", count)
                .arg("drivers", create.drivers.len()));
        }
        // As in the CLI: one master seed determines the match and every bot.
        let mut master = GameRng::from_seed(Seed::from_u64(create.seed));
        let match_seed = derive_seed(&mut master);
        let bots = (0..count)
            .map(|_| RandomBot::new(derive_seed(&mut master)))
            .collect();
        let search: Vec<SearchBot<G>> = match search {
            Some(make) => (0..count).map(|_| make(derive_seed(&mut master))).collect(),
            None => Vec::new(),
        };
        if search.is_empty() && create.drivers.contains(&Driver::Ismcts) {
            return Err(ServerError::new("server-error-no-ismcts"));
        }
        let session =
            Session::<G>::new(config, create.seats, match_seed, create.build).map_err(|error| {
                ServerError::new("server-error-invalid-variant").reason(error.reason)
            })?;
        let mut hosted = Self {
            session,
            drivers: create.drivers,
            drafts: vec![Vec::new(); count],
            bots,
            search,
        };
        hosted.run_bots()?;
        Ok(hosted)
    }

    fn seat(&self, index: u8) -> Result<Seat, ServerError> {
        Seat::new(index)
            .filter(|seat| self.session.seats().contains(*seat))
            .ok_or_else(|| ServerError::new("server-error-bad-seat").arg("seat", index))
    }

    fn human(&self, index: u8) -> Result<Seat, ServerError> {
        let seat = self.seat(index)?;
        if self.drivers[usize::from(index)].is_bot() {
            return Err(ServerError::new("server-error-seat-is-bot").arg("seat", index));
        }
        Ok(seat)
    }

    /// Lets bot seats act until a human must decide or the match ends.
    fn run_bots(&mut self) -> Result<Vec<Moved>, ServerError> {
        let mut moved = Vec::new();
        for _ in 0..MAX_BOT_MOVES {
            let pending = self.session.pending().seats();
            let Some(seat) = pending
                .iter()
                .find(|seat| self.drivers[usize::from(seat.index())].is_bot())
            else {
                break;
            };
            let index = usize::from(seat.index());
            let bot: &mut dyn Player<G> = match self.drivers[index] {
                Driver::Ismcts => self.search[index].as_mut(),
                _ => &mut self.bots[index],
            };
            let events = self.session.play(seat, bot).map_err(rejected)?;
            moved.push(self.moved(seat, &events));
        }
        Ok(moved)
    }

    fn moved(&mut self, seat: Seat, events: &[G::Event]) -> Moved {
        self.prune_drafts();
        Moved {
            seat: seat.index(),
            position: self.session.record().moves.len(),
            events: events
                .iter()
                .map(|event| serde_json::to_value(event).unwrap_or(Value::Null))
                .collect(),
        }
    }

    /// After a move, keeps each draft only while it still leads somewhere.
    fn prune_drafts(&mut self) {
        for (index, draft) in self.drafts.iter_mut().enumerate() {
            let Some(seat) = u8::try_from(index).ok().and_then(Seat::new) else {
                continue;
            };
            if self.session.prompt(seat, draft).is_err() {
                draft.clear();
            }
        }
    }

    /// The record with only its first `position` moves.
    fn prefix(&self, position: usize) -> RecordOf<G> {
        let mut record = self.session.record().clone();
        record.moves.truncate(position);
        record
    }

    fn log(&self, observers: &[(WireObserver, Observer)]) -> Result<Vec<LogEntry>, ServerError> {
        let sees = |seat: Seat| {
            observers
                .iter()
                .any(|(_, observer)| observer.seat() == Some(seat) || observer.sees_all())
        };
        let mut replay = Session::<G>::from_record(self.prefix(0)).map_err(internal)?;
        let mut log = Vec::with_capacity(self.len());
        for recorded in &self.session.record().moves {
            let seat = recorded.seat;
            let choices =
                sees(seat).then(|| G::choices(&replay.view(Observer::Seat(seat)), &recorded.mv));
            log.push(LogEntry {
                seat: seat.index(),
                choices,
            });
            replay
                .submit_move(seat, recorded.mv.clone())
                .map_err(|_| internal("replay"))?;
        }
        Ok(log)
    }

    fn render(
        &self,
        session: &Session<G>,
        observers: &[(WireObserver, Observer)],
        live: bool,
    ) -> Snapshot {
        let pending = session.pending();
        let prompts = if live {
            observers
                .iter()
                .filter_map(|(_, observer)| observer.seat())
                .filter(|seat| {
                    pending.allows(*seat)
                        && self.drivers[usize::from(seat.index())] == Driver::Human
                })
                .filter_map(|seat| {
                    let made = &self.drafts[usize::from(seat.index())];
                    let prompt = session.prompt(seat, made).ok()?;
                    Some(WirePrompt {
                        seat: seat.index(),
                        made: made.clone(),
                        selectable: prompt.selectable,
                        complete: prompt.complete,
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        Snapshot {
            position: session.record().moves.len(),
            length: self.len(),
            pending: wire_pending(pending),
            drivers: self.drivers.clone(),
            views: observers
                .iter()
                .map(|(wire, observer)| ObserverView {
                    observer: *wire,
                    table: session.table(*observer),
                })
                .collect(),
            prompts,
            log: Vec::new(),
            outcome: session.outcome().map(|outcome| WireOutcome {
                seats: outcome.seats,
                teams: outcome
                    .teams
                    .iter()
                    .map(|team| team.iter().map(Seat::index).collect())
                    .collect(),
            }),
        }
    }
}

impl<G: RegisteredGame> Hosted for Match<G> {
    fn len(&self) -> usize {
        self.session.record().moves.len()
    }

    fn snapshot(
        &self,
        observers: &[(WireObserver, Observer)],
        position: Option<usize>,
    ) -> Result<Snapshot, ServerError> {
        let length = self.len();
        let mut snapshot = match position {
            None => self.render(&self.session, observers, true),
            Some(position) if position > length => {
                return Err(ServerError::new("server-error-bad-position")
                    .arg("position", position)
                    .arg("length", length));
            }
            Some(position) => {
                // Replay is deterministic, so the past is rebuilt from the
                // record rather than stored.
                let past = Session::<G>::from_record(self.prefix(position)).map_err(internal)?;
                self.render(&past, observers, false)
            }
        };
        snapshot.log = self.log(observers)?;
        Ok(snapshot)
    }

    fn choose(&mut self, seat: u8, choice: Choice) -> Result<(), ServerError> {
        let seat = self.human(seat)?;
        let draft = &self.drafts[usize::from(seat.index())];
        let mut made = draft.clone();
        made.push(choice);
        self.session.prompt(seat, &made).map_err(rejected)?;
        self.drafts[usize::from(seat.index())] = made;
        Ok(())
    }

    fn undo(&mut self, seat: u8) -> Result<(), ServerError> {
        let seat = self.human(seat)?;
        self.drafts[usize::from(seat.index())].pop();
        Ok(())
    }

    fn submit(&mut self, seat: u8) -> Result<Vec<Moved>, ServerError> {
        let seat = self.human(seat)?;
        let index = usize::from(seat.index());
        let events = self
            .session
            .submit_choices(seat, &self.drafts[index])
            .map_err(rejected)?;
        self.drafts[index].clear();
        let mut moved = vec![self.moved(seat, &events)];
        moved.extend(self.run_bots()?);
        Ok(moved)
    }

    fn set_driver(&mut self, seat: u8, driver: Driver) -> Result<Vec<Moved>, ServerError> {
        let seat = self.seat(seat)?;
        let index = usize::from(seat.index());
        if driver == Driver::Ismcts && self.search.is_empty() {
            return Err(ServerError::new("server-error-no-ismcts"));
        }
        self.drivers[index] = driver;
        if driver.is_bot() {
            self.drafts[index].clear();
        }
        self.run_bots()
    }

    fn record(&self) -> Value {
        serde_json::to_value(self.session.record()).unwrap_or(Value::Null)
    }
}

fn wire_pending(pending: Pending) -> WirePending {
    let kind = match pending {
        Pending::All(_) => PendingKind::All,
        Pending::Any(_) => PendingKind::Any,
        // The session resolves chance steps itself, so `Chance` never shows.
        Pending::Chance | Pending::Finished => PendingKind::Finished,
    };
    WirePending {
        kind,
        seats: pending.seats().iter().map(Seat::index).collect(),
    }
}

fn rejected(error: SubmitError) -> ServerError {
    match error {
        SubmitError::NotPending(seat) => {
            ServerError::new("server-error-not-pending").arg("seat", seat.index())
        }
        SubmitError::NotSelectable { .. } => ServerError::new("server-error-not-selectable"),
        SubmitError::Incomplete => ServerError::new("server-error-incomplete"),
        SubmitError::Ambiguous(_) => ServerError::new("server-error-ambiguous"),
        SubmitError::Illegal(error) => {
            ServerError::new("server-error-illegal").reason(error.reason)
        }
    }
}

fn internal(detail: impl core::fmt::Debug) -> ServerError {
    ServerError::new("server-error-internal").arg("detail", format!("{detail:?}"))
}
