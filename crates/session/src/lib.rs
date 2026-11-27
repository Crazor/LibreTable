//! Match flow for one match (architecture: sessions, transports &
//! multiplayer).
//!
//! A [`Session`] owns the authoritative state, the match's [`GameRng`] and
//! the [`GameRecord`](libretable_game_core::GameRecord) it writes. It is a
//! step-by-step API that a front end drives: ask who must act
//! ([`pending`](Session::pending)), show a seat its table and what it may
//! choose ([`table`](Session::table), [`prompt`](Session::prompt)), and
//! submit a complete choice path ([`submit_choices`](Session::submit_choices)).
//! Chance steps are resolved inside the session, so outside it the match is
//! always waiting for a seat or finished.
//!
//! Who sits at a seat (a local human, a bot, a remote client) is the front
//! end's business; bots get a [`Decision`] built from the seat's own view and
//! play through [`play`](Session::play). Front ends never see the state:
//! only views, table models, choices and the record leave the session.
//!
//! The crate is `no_std` and has no I/O, clock or threads, so the CLI, the
//! dev server and later the apps run identical match logic. Timeouts and
//! transports will live around it.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

pub use libretable_bots::{Decision, Player};
use libretable_game_core::{
    Choice, Game, GameRng, IllegalMove, InvalidVariant, Observer, Outcome, Pending, RecordOf,
    ReplayError, Seat, SeatCount, Seed, Selection, SelectionError, TableModel, Variant, replay,
};

/// One running match of game `G`.
///
/// `Debug` prints only public facts (game, seats, moves, pending), never the
/// state. Two sessions are equal when state, generator and record are, so a
/// session restored with [`from_record`](Session::from_record) compares equal
/// to the original.
pub struct Session<G: Game> {
    state: G::State,
    rng: GameRng,
    record: RecordOf<G>,
}

// Manual impls: a derive would require `G` itself to be `Clone` / `Eq`.
impl<G: Game> Clone for Session<G> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            rng: self.rng.clone(),
            record: self.record.clone(),
        }
    }
}

impl<G: Game> PartialEq for Session<G> {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state && self.rng == other.rng && self.record == other.record
    }
}

impl<G: Game> Eq for Session<G> {}

/// What a seat may choose next, given the choices it has made so far.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    /// The next selectable choices, without duplicates, in the game's move
    /// order. Empty once `complete`.
    pub selectable: Vec<Choice>,
    /// Whether the choices made form a legal move's whole path, so they can be
    /// submitted with [`Session::submit_choices`].
    pub complete: bool,
}

/// Why a submission or prompt was rejected. The session is unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubmitError {
    /// The seat is outside the match or may not act now.
    NotPending(Seat),
    /// The choice at this position of the path is not selectable.
    NotSelectable {
        /// Position in the submitted path.
        index: usize,
    },
    /// The path is a proper prefix of a legal move's path.
    Incomplete,
    /// The game's choice paths for this seat are not unique and prefix-free
    /// (a bug the conformance suite catches).
    Ambiguous(SelectionError),
    /// The game rejected the move.
    Illegal(IllegalMove),
}

impl fmt::Display for SubmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotPending(seat) => write!(f, "seat {} may not act now", seat.index()),
            Self::NotSelectable { index } => write!(f, "choice {index} is not selectable"),
            Self::Incomplete => f.write_str("the choices do not form a complete move"),
            Self::Ambiguous(error) => write!(f, "ambiguous choice paths: {error:?}"),
            Self::Illegal(error) => write!(f, "illegal move: {}", error.reason),
        }
    }
}

impl<G: Game> Session<G> {
    /// Starts a match: validates the variant, sets up from `seed` and
    /// resolves any initial chance step. `build` identifies the binary that
    /// runs the match and goes into the record (ADR-0004).
    ///
    /// Events of chance steps resolved here are not reported.
    ///
    /// # Errors
    ///
    /// [`InvalidVariant`] if `config` is invalid for `seats`.
    pub fn new(
        config: G::Config,
        seats: SeatCount,
        seed: Seed,
        build: impl Into<String>,
    ) -> Result<Self, InvalidVariant> {
        config.validate(seats)?;
        let mut rng = GameRng::from_seed(seed);
        let mut state = G::setup(&config, seats, &mut rng);
        resolve_chance::<G>(&mut state, &mut rng);
        Ok(Self {
            state,
            rng,
            record: RecordOf::<G>::new(G::ID, build, config, seats, seed),
        })
    }

    /// Restores a match from its record by replaying it; the match continues
    /// from where the record ends and appends to it.
    ///
    /// # Errors
    ///
    /// [`ReplayError`] if the record does not belong to `G` or does not
    /// replay.
    pub fn from_record(record: RecordOf<G>) -> Result<Self, ReplayError> {
        let (state, rng) = replay::<G>(&record)?;
        Ok(Self { state, rng, record })
    }

    /// Number of seats.
    #[must_use]
    pub fn seats(&self) -> SeatCount {
        self.record.seats
    }

    /// Who must act now: `All`, `Any` or `Finished`, never `Chance`.
    #[must_use]
    pub fn pending(&self) -> Pending {
        G::pending(&self.state)
    }

    /// The result once the match is finished.
    #[must_use]
    pub fn outcome(&self) -> Option<Outcome> {
        G::outcome(&self.state)
    }

    /// The view of `observer`. Send a seat only its own view; a non-dev build
    /// cannot ask for `Observer::Omniscient` at all.
    #[must_use]
    pub fn view(&self, observer: Observer) -> G::View {
        G::view(&self.state, observer)
    }

    /// The table model of `observer`'s view.
    #[must_use]
    pub fn table(&self, observer: Observer) -> TableModel {
        G::table(&self.view(observer))
    }

    /// The record so far: config, seats, seed and every applied move.
    #[must_use]
    pub fn record(&self) -> &RecordOf<G> {
        &self.record
    }

    /// Ends the session and returns its record.
    #[must_use]
    pub fn into_record(self) -> RecordOf<G> {
        self.record
    }

    /// What `seat` must decide now, from its own view, for a [`Player`].
    /// `None` if the seat may not act.
    #[must_use]
    pub fn decision(&self, seat: Seat) -> Option<Decision<G::View>> {
        if !self.may_act(seat) {
            return None;
        }
        let view = self.view(Observer::Seat(seat));
        let paths = G::legal_moves(&self.state, seat)
            .iter()
            .map(|mv| G::choices(&view, mv))
            .collect();
        Some(Decision { seat, view, paths })
    }

    /// The selection protocol for clients: what `seat` may choose after the
    /// choices `made` so far. A client keeps its own choices (undo is
    /// dropping the last one) and asks again after each.
    ///
    /// # Errors
    ///
    /// [`SubmitError::NotPending`], [`SubmitError::NotSelectable`] if `made`
    /// is not a prefix of a legal path, or [`SubmitError::Ambiguous`].
    pub fn prompt(&self, seat: Seat, made: &[Choice]) -> Result<Prompt, SubmitError> {
        let selection = self.select(seat, made)?;
        Ok(Prompt {
            selectable: selection.selectable(),
            complete: selection.complete().is_some(),
        })
    }

    /// Submits the move whose whole choice path is `choices`, applies it,
    /// records it and resolves the chance steps that follow. Returns the
    /// events of the move and of those chance steps.
    ///
    /// # Errors
    ///
    /// [`SubmitError`]; the session is then unchanged.
    pub fn submit_choices(
        &mut self,
        seat: Seat,
        choices: &[Choice],
    ) -> Result<Vec<G::Event>, SubmitError> {
        let mv = self
            .select(seat, choices)?
            .into_move()
            .ok_or(SubmitError::Incomplete)?;
        self.submit_move(seat, mv)
    }

    /// Lets `player` decide for `seat` and submits its choice path.
    ///
    /// # Errors
    ///
    /// [`SubmitError::NotPending`] if the seat may not act, or the error of
    /// [`submit_choices`](Self::submit_choices) if the player returned a path
    /// that is not legal.
    pub fn play(
        &mut self,
        seat: Seat,
        player: &mut dyn Player<G>,
    ) -> Result<Vec<G::Event>, SubmitError> {
        let decision = self.decision(seat).ok_or(SubmitError::NotPending(seat))?;
        let path = player.decide(&decision);
        self.submit_choices(seat, &path)
    }

    /// Submits a move as data, bypassing the selection protocol. For trusted
    /// sources that already hold moves (records, tests); clients and bots use
    /// [`submit_choices`](Self::submit_choices) or [`play`](Self::play).
    ///
    /// # Errors
    ///
    /// [`SubmitError::NotPending`] or [`SubmitError::Illegal`]; the session is
    /// then unchanged.
    pub fn submit_move(&mut self, seat: Seat, mv: G::Move) -> Result<Vec<G::Event>, SubmitError> {
        if !self.may_act(seat) {
            return Err(SubmitError::NotPending(seat));
        }
        let mut events =
            G::apply(&mut self.state, seat, &mv, &mut self.rng).map_err(SubmitError::Illegal)?;
        self.record.push(seat, mv);
        events.extend(resolve_chance::<G>(&mut self.state, &mut self.rng));
        Ok(events)
    }

    fn may_act(&self, seat: Seat) -> bool {
        self.record.seats.contains(seat) && self.pending().allows(seat)
    }

    fn select(&self, seat: Seat, made: &[Choice]) -> Result<Selection<G::Move>, SubmitError> {
        if !self.may_act(seat) {
            return Err(SubmitError::NotPending(seat));
        }
        let mut selection =
            Selection::new::<G>(&self.state, seat).map_err(SubmitError::Ambiguous)?;
        for (index, choice) in made.iter().enumerate() {
            selection
                .choose(choice.clone())
                .map_err(|_| SubmitError::NotSelectable { index })?;
        }
        Ok(selection)
    }
}

impl<G: Game> fmt::Debug for Session<G> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("game", &G::ID)
            .field("seats", &self.record.seats)
            .field("moves", &self.record.moves.len())
            .field("pending", &self.pending())
            .finish_non_exhaustive()
    }
}

/// Draws a fresh seed from `rng`, e.g. to derive match and bot seeds from one
/// master seed so that a whole simulation is reproducible.
#[must_use]
pub fn derive_seed(rng: &mut GameRng) -> Seed {
    let mut bytes = [0; 32];
    for chunk in bytes.chunks_exact_mut(8) {
        chunk.copy_from_slice(&rng.next_u64().to_le_bytes());
    }
    Seed(bytes)
}

fn resolve_chance<G: Game>(state: &mut G::State, rng: &mut GameRng) -> Vec<G::Event> {
    let mut events = Vec::new();
    while G::pending(state) == Pending::Chance {
        events.extend(G::apply_chance(state, rng));
    }
    events
}
