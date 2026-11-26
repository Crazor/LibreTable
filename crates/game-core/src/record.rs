//! Game records: saves, replays and network sync share this format.

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::{
    Game, GameId, GameRng, IllegalMove, InvalidVariant, Pending, Seat, SeatCount, Seed, Variant,
};

/// Schema version written into every [`GameRecord`] (ADR-0003). Bumped on any
/// incompatible change to the record layout or to the meaning of its fields.
pub const RECORD_SCHEMA_VERSION: u16 = 1;

/// A complete match: `(game ID, variant, seats, seed, move list)` plus the
/// build that produced it (ADR-0004).
///
/// Chance steps are not recorded; they follow from the seed. Moves are stored
/// in the order the authoritative session applied them, so simultaneous
/// ([`Pending::All`]) and real-time ([`Pending::Any`]) decisions replay
/// exactly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameRecord<C, M> {
    /// [`RECORD_SCHEMA_VERSION`] at the time of writing.
    pub schema: u16,
    /// The game's [`GameId`].
    pub game: String,
    /// Build ID (git commit) that produced the record. Diagnostics only.
    pub build: String,
    /// The variant the match was played with.
    pub variant: C,
    /// Number of seats.
    pub seats: SeatCount,
    /// Seed of the match's [`GameRng`].
    pub seed: Seed,
    /// Applied moves, in order.
    pub moves: Vec<RecordedMove<M>>,
}

/// The record type of game `G`.
pub type RecordOf<G> = GameRecord<<G as Game>::Config, <G as Game>::Move>;

/// One applied move.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedMove<M> {
    /// The seat that made the move.
    pub seat: Seat,
    /// The move.
    #[serde(rename = "move")]
    pub mv: M,
}

impl<C, M> GameRecord<C, M> {
    /// An empty record with the current schema version.
    pub fn new(
        game: GameId,
        build: impl Into<String>,
        variant: C,
        seats: SeatCount,
        seed: Seed,
    ) -> Self {
        Self {
            schema: RECORD_SCHEMA_VERSION,
            game: game.as_str().into(),
            build: build.into(),
            variant,
            seats,
            seed,
            moves: Vec::new(),
        }
    }

    /// Appends an applied move.
    pub fn push(&mut self, seat: Seat, mv: M) {
        self.moves.push(RecordedMove { seat, mv });
    }
}

/// Why [`replay`] failed. `index` is the position in [`GameRecord::moves`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplayError {
    /// The record was written with another schema version.
    Schema {
        /// The record's version.
        found: u16,
    },
    /// The record belongs to another game.
    WrongGame,
    /// The variant or seat count is invalid.
    InvalidVariant(InvalidVariant),
    /// A move was recorded for a seat that may not act at that point.
    NotPending {
        /// Position of the move.
        index: usize,
    },
    /// The game rejected a move.
    IllegalMove {
        /// Position of the move.
        index: usize,
        /// The game's reason.
        error: IllegalMove,
    },
}

/// Replays `record` and returns the reached state and the generator, ready to
/// continue the match.
///
/// This is the reference match loop: set up from the seed, resolve every
/// [`Pending::Chance`] as it comes up (also after the last move), and apply
/// each recorded move, checking that its seat is pending.
///
/// # Errors
///
/// [`ReplayError`] if the record does not belong to `G` or a move does not
/// apply. Such a record is outdated (ADR-0004) or corrupt.
pub fn replay<G: Game>(record: &RecordOf<G>) -> Result<(G::State, GameRng), ReplayError> {
    if record.schema != RECORD_SCHEMA_VERSION {
        return Err(ReplayError::Schema {
            found: record.schema,
        });
    }
    if record.game != G::ID.as_str() {
        return Err(ReplayError::WrongGame);
    }
    record
        .variant
        .validate(record.seats)
        .map_err(ReplayError::InvalidVariant)?;

    let mut rng = GameRng::from_seed(record.seed);
    let mut state = G::setup(&record.variant, record.seats, &mut rng);
    resolve_chance::<G>(&mut state, &mut rng);
    for (index, RecordedMove { seat, mv }) in record.moves.iter().enumerate() {
        if !record.seats.contains(*seat) || !G::pending(&state).allows(*seat) {
            return Err(ReplayError::NotPending { index });
        }
        G::apply(&mut state, *seat, mv, &mut rng)
            .map_err(|error| ReplayError::IllegalMove { index, error })?;
        resolve_chance::<G>(&mut state, &mut rng);
    }
    Ok((state, rng))
}

fn resolve_chance<G: Game>(state: &mut G::State, rng: &mut GameRng) {
    while G::pending(state) == Pending::Chance {
        G::apply_chance(state, rng);
    }
}
