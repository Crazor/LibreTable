//! Players that act from views only (architecture: bots).
//!
//! A [`Player`] gets a [`Decision`]: the seat's own view and the choice paths
//! of its legal moves (selection protocol), the same information a human at
//! that seat has. It answers with one of the paths. It never sees the state,
//! so it cannot use hidden information, and the session never builds a
//! decision from `Observer::Omniscient`.
//!
//! [`RandomBot`] picks a legal move uniformly at random. The ISMCTS bot
//! (milestone 1, issue 9) will implement the same trait.
//!
//! The crate is `no_std`: bots use no I/O, clock, threads or global
//! randomness; a bot's randomness comes from its own [`GameRng`].

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::vec::Vec;

use libretable_game_core::{Choice, Game, GameRng, Seat, Seed};

/// What a seat must decide now, as that seat sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision<V> {
    /// The deciding seat.
    pub seat: Seat,
    /// The seat's own view (`Observer::Seat(seat)`).
    pub view: V,
    /// The choice path of every legal move, in the order the game lists the
    /// moves. Unique and prefix-free; never empty for a pending seat.
    pub paths: Vec<Vec<Choice>>,
}

/// Anything that decides for a seat: a bot now, a remote human later.
///
/// Same interface on every platform (architecture: "bots are just players").
pub trait Player<G: Game> {
    /// Picks a move for `decision.seat` and returns its choice path, which
    /// must be one of `decision.paths`.
    fn decide(&mut self, decision: &Decision<G::View>) -> Vec<Choice>;
}

/// Plays a uniformly random legal move.
///
/// Deterministic for a given seed and sequence of decisions, so simulations
/// and bot-vs-bot matches replay exactly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RandomBot {
    rng: GameRng,
}

impl RandomBot {
    /// A random bot drawing from its own generator seeded with `seed`. Use a
    /// seed different from the match seed, so the bot's choices do not
    /// correlate with the game's chance.
    #[must_use]
    pub fn new(seed: Seed) -> Self {
        Self {
            rng: GameRng::from_seed(seed),
        }
    }
}

impl<G: Game> Player<G> for RandomBot {
    fn decide(&mut self, decision: &Decision<G::View>) -> Vec<Choice> {
        // A seat has far fewer than 2^32 legal moves; saturate defensively.
        let count = u32::try_from(decision.paths.len()).unwrap_or(u32::MAX);
        if count == 0 {
            return Vec::new();
        }
        let index = self.rng.below(count) as usize;
        decision.paths[index].clone()
    }
}
