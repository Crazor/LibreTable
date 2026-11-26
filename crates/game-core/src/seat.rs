//! Seats, seat sets and observers.

use core::fmt;

use serde::{Deserialize, Serialize};

/// Maximum number of seats in one match. [`SeatSet`] is a 64-bit mask.
pub const MAX_SEATS: u8 = 64;

/// A place at the table, numbered from 0 in seating order.
///
/// Who occupies a seat (human, bot, remote player) is session business; the
/// core only ever sees seats.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Seat(u8);

impl Seat {
    /// Returns the seat with the given index, or `None` if it is not below
    /// [`MAX_SEATS`].
    #[must_use]
    pub const fn new(index: u8) -> Option<Self> {
        if index < MAX_SEATS {
            Some(Self(index))
        } else {
            None
        }
    }

    /// The seat's index, `0..MAX_SEATS`.
    #[must_use]
    pub const fn index(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Seat {
    type Error = &'static str;

    fn try_from(index: u8) -> Result<Self, Self::Error> {
        Self::new(index).ok_or("seat index out of range")
    }
}

impl From<Seat> for u8 {
    fn from(seat: Seat) -> Self {
        seat.0
    }
}

impl fmt::Debug for Seat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Seat({})", self.0)
    }
}

/// Number of seats in a match, `1..=MAX_SEATS`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct SeatCount(u8);

impl SeatCount {
    /// Returns the seat count, or `None` if it is zero or above [`MAX_SEATS`].
    #[must_use]
    pub const fn new(count: u8) -> Option<Self> {
        if count >= 1 && count <= MAX_SEATS {
            Some(Self(count))
        } else {
            None
        }
    }

    /// The number of seats.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }

    /// All seats of the match, in seating order.
    pub fn seats(self) -> impl DoubleEndedIterator<Item = Seat> + ExactSizeIterator {
        (0..self.0).map(Seat)
    }

    /// The set of all seats of the match.
    #[must_use]
    pub const fn all(self) -> SeatSet {
        if self.0 == MAX_SEATS {
            SeatSet(u64::MAX)
        } else {
            SeatSet((1 << self.0) - 1)
        }
    }

    /// Whether `seat` belongs to a match of this size.
    #[must_use]
    pub const fn contains(self, seat: Seat) -> bool {
        seat.0 < self.0
    }
}

impl TryFrom<u8> for SeatCount {
    type Error = &'static str;

    fn try_from(count: u8) -> Result<Self, Self::Error> {
        Self::new(count).ok_or("seat count out of range")
    }
}

impl From<SeatCount> for u8 {
    fn from(count: SeatCount) -> Self {
        count.0
    }
}

impl fmt::Debug for SeatCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SeatCount({})", self.0)
    }
}

/// A set of seats, stored as a bit mask (bit `n` = seat `n`).
///
/// `Copy` and allocation-free, because [`Pending`](crate::Pending) is queried
/// on every step of every bot playout.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SeatSet(u64);

impl SeatSet {
    /// The empty set.
    pub const EMPTY: Self = Self(0);

    /// The set containing only `seat`.
    #[must_use]
    pub const fn single(seat: Seat) -> Self {
        Self(1 << seat.0)
    }

    /// The raw bit mask.
    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Whether the set is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Number of seats in the set.
    #[must_use]
    pub const fn len(self) -> u32 {
        self.0.count_ones()
    }

    /// Whether `seat` is in the set.
    #[must_use]
    pub const fn contains(self, seat: Seat) -> bool {
        self.0 & (1 << seat.0) != 0
    }

    /// The set with `seat` added.
    #[must_use]
    pub const fn with(self, seat: Seat) -> Self {
        Self(self.0 | (1 << seat.0))
    }

    /// The set with `seat` removed.
    #[must_use]
    pub const fn without(self, seat: Seat) -> Self {
        Self(self.0 & !(1 << seat.0))
    }

    /// Seats in both sets.
    #[must_use]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Seats in either set.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// The seats in ascending order.
    pub fn iter(self) -> impl Iterator<Item = Seat> {
        let mut bits = self.0;
        core::iter::from_fn(move || {
            if bits == 0 {
                return None;
            }
            // trailing_zeros of a non-zero u64 is below 64, so it fits a seat.
            let index = bits.trailing_zeros() as u8;
            bits &= bits - 1;
            Some(Seat(index))
        })
    }
}

impl FromIterator<Seat> for SeatSet {
    fn from_iter<I: IntoIterator<Item = Seat>>(iter: I) -> Self {
        iter.into_iter().fold(Self::EMPTY, Self::with)
    }
}

impl fmt::Debug for SeatSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter().map(Seat::index)).finish()
    }
}

/// Who a [`View`](crate::Game::View) is built for.
///
/// A view contains exactly what its observer may know (architecture: views &
/// hidden information).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Observer {
    /// The player at this seat: own hidden components, public zones, counts.
    Seat(Seat),
    /// Public information only.
    Spectator,
    /// The full state. Local development builds and tests only; a non-dev
    /// server never builds or sends this view.
    #[cfg(feature = "dev")]
    Omniscient,
}
