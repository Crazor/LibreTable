//! Who must decide now.

use serde::{Deserialize, Serialize};

use crate::{Seat, SeatSet};

/// The decision a state is waiting for, as returned by
/// [`Game::pending`](crate::Game::pending).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Pending {
    /// Every listed seat submits exactly one move; the moves resolve together
    /// (sealed bids, card passing, rock-paper-scissors).
    ///
    /// Moves are still applied one at a time in arrival order: the game keeps
    /// the sealed moves in its state, removes each seat that has submitted
    /// from the set and resolves once the last move arrives. Views must not
    /// reveal sealed moves before resolution.
    All(SeatSet),
    /// Any listed seat may act at any time; moves apply in arrival order and a
    /// move made illegal by a faster one is rejected (Ligretto, Nerts).
    Any(SeatSet),
    /// A chance step (dealing, a die roll) must be resolved with
    /// [`Game::apply_chance`](crate::Game::apply_chance) before any seat may act.
    Chance,
    /// The game is over; [`Game::outcome`](crate::Game::outcome) is `Some`.
    Finished,
}

impl Pending {
    /// The seats allowed to submit a move now; empty for `Chance` and
    /// `Finished`.
    #[must_use]
    pub const fn seats(self) -> SeatSet {
        match self {
            Self::All(seats) | Self::Any(seats) => seats,
            Self::Chance | Self::Finished => SeatSet::EMPTY,
        }
    }

    /// Whether `seat` may submit a move now.
    #[must_use]
    pub const fn allows(self, seat: Seat) -> bool {
        self.seats().contains(seat)
    }
}
