//! The `Game` trait and its companion types.

use alloc::vec::Vec;
use core::fmt;

use serde::{Serialize, de::DeserializeOwned};

use crate::{GameRng, Observer, Outcome, Pending, Seat, SeatCount};

/// Stable identifier of a game, e.g. `"mau-mau"`.
///
/// Lowercase ASCII letters, digits and single inner hyphens. It is an
/// internal ID and never localized.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GameId(&'static str);

impl GameId {
    /// Creates a game ID.
    ///
    /// # Panics
    ///
    /// If `id` is empty, contains anything but `a-z`, `0-9` and `-`, or
    /// starts, ends or repeats a hyphen. In a `const` this fails the build.
    #[must_use]
    pub const fn new(id: &'static str) -> Self {
        let bytes = id.as_bytes();
        assert!(!bytes.is_empty(), "game ID must not be empty");
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            let hyphen = b == b'-';
            assert!(
                b.is_ascii_lowercase() || b.is_ascii_digit() || hyphen,
                "game ID may contain only a-z, 0-9 and '-'"
            );
            if hyphen {
                assert!(
                    i > 0 && i + 1 < bytes.len() && bytes[i + 1] != b'-',
                    "game ID hyphens must be single and inside the ID"
                );
            }
            i += 1;
        }
        Self(id)
    }

    /// The ID as a string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Debug for GameId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GameId({:?})", self.0)
    }
}

impl fmt::Display for GameId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// A validated variant: the standard ruleset plus typed house-rule options.
///
/// `Default` is the game's standard ruleset.
pub trait Variant:
    Clone + fmt::Debug + PartialEq + Eq + Default + Serialize + DeserializeOwned
{
    /// Checks the option combination and the seat count. An invalid variant is
    /// rejected before [`Game::setup`] runs.
    ///
    /// # Errors
    ///
    /// [`InvalidVariant`] naming the first problem found.
    fn validate(&self, seats: SeatCount) -> Result<(), InvalidVariant>;
}

/// Why a variant was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InvalidVariant {
    /// Fluent message ID describing the problem, e.g. `"mau-mau-too-many-seats"`.
    pub reason: &'static str,
}

/// Why a move was rejected by [`Game::apply`]. The state is unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IllegalMove {
    /// Fluent message ID describing the problem, e.g. `"move-not-your-turn"`.
    pub reason: &'static str,
}

/// A game as a pure, deterministic state machine.
///
/// A match is fully defined by config, seat count, seed and move list: the
/// session creates a [`GameRng`] from the seed, calls [`setup`](Self::setup)
/// and then, until [`pending`](Self::pending) is [`Pending::Finished`],
/// resolves [`Pending::Chance`] with [`apply_chance`](Self::apply_chance) and
/// otherwise applies the next submitted move with [`apply`](Self::apply).
/// [`replay`](crate::replay) is the reference implementation of that loop.
///
/// Implementations must not use I/O, time, threads or any randomness except
/// the `rng` they are handed.
pub trait Game {
    /// Validated variant / house-rule set.
    type Config: Variant;
    /// Full, authoritative state. Cheap to clone; bots clone it per playout.
    type State: Clone + fmt::Debug + PartialEq + Eq + Serialize + DeserializeOwned;
    /// One decision of one seat, as plain data.
    type Move: Clone + fmt::Debug + PartialEq + Eq + Serialize + DeserializeOwned;
    /// What one observer may know about the state.
    type View: Clone + fmt::Debug + PartialEq + Eq + Serialize + DeserializeOwned;
    /// What happened during one step (card moved, revealed, shuffled, trick
    /// won). Renderers animate from events; logs and replays read them. An
    /// event must reveal no more than the view of the seat it is sent to.
    type Event: Clone + fmt::Debug + PartialEq + Eq + Serialize + DeserializeOwned;

    /// Stable game ID.
    const ID: GameId;

    /// Creates the initial state. `config` has passed
    /// [`Variant::validate`] for `seats`.
    fn setup(config: &Self::Config, seats: SeatCount, rng: &mut GameRng) -> Self::State;

    /// Who must decide now. Never `All` or `Any` with an empty set.
    fn pending(state: &Self::State) -> Pending;

    /// All moves `seat` may submit now; empty if `seat` is not pending.
    fn legal_moves(state: &Self::State, seat: Seat) -> Vec<Self::Move>;

    /// Applies one move of `seat`. Chance inside a move (drawing from a
    /// shuffled pile) uses `rng` only.
    ///
    /// # Errors
    ///
    /// [`IllegalMove`] if `seat` is not pending or `mv` is not legal for it;
    /// the state is then unchanged.
    fn apply(
        state: &mut Self::State,
        seat: Seat,
        mv: &Self::Move,
        rng: &mut GameRng,
    ) -> Result<Vec<Self::Event>, IllegalMove>;

    /// Resolves a pending chance step (dealing, a die roll) using `rng` only.
    /// Called only while [`pending`](Self::pending) is [`Pending::Chance`].
    ///
    /// The default panics; a game whose `pending` ever returns
    /// [`Pending::Chance`] must override it.
    fn apply_chance(state: &mut Self::State, rng: &mut GameRng) -> Vec<Self::Event> {
        let _ = (state, rng);
        panic!(
            "{} returned Pending::Chance but does not implement apply_chance",
            Self::ID
        )
    }

    /// The view of `observer`: exactly what it may know, nothing more.
    fn view(state: &Self::State, observer: Observer) -> Self::View;

    /// The result once the game is finished; `None` before.
    fn outcome(state: &Self::State) -> Option<Outcome>;
}
