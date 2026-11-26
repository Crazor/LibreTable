//! Final results of a match.

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::{Seat, SeatSet};

/// Result of a finished match, per seat and per team.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Outcome {
    /// One entry per seat, indexed by [`Seat::index`].
    pub seats: Vec<SeatResult>,
    /// Partnerships as a partition of the seats (Doppelkopf, Canasta, Skat's
    /// declarer vs. defenders). Empty when every seat plays for itself.
    /// Partners share a rank.
    pub teams: Vec<SeatSet>,
}

impl Outcome {
    /// The result of `seat`, if the seat took part.
    #[must_use]
    pub fn result(&self, seat: Seat) -> Option<&SeatResult> {
        self.seats.get(usize::from(seat.index()))
    }
}

/// Result of one seat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SeatResult {
    /// Final placing, 1 = best. Tied seats share a rank; the next rank skips
    /// accordingly (1, 1, 3). An eliminated seat ranks by elimination order.
    pub rank: u8,
    /// Final score in the game's own unit (points, penalty points, chips).
    /// Games without scores report 0. Whether higher is better is the game's
    /// business; `rank` is authoritative for winning.
    pub score: i64,
}
