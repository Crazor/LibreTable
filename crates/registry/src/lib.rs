//! The games front ends know, by ID.
//!
//! Front ends (the CLI, the server, later the bindings) are generic over
//! [`Game`]; this crate lets them pick a game by its ID at run time. A front
//! end implements [`Visitor`] once and gets its generic code called with the
//! game's type: [`visit`] for one game, [`GAMES`] for the list.
//!
//! Adding a game is one line in the `games!` invocation below.
//!
//! ```
//! use libretable_game_core::Game;
//! use libretable_registry::{RegisteredGame, Visitor, visit};
//!
//! struct DefaultVariant;
//! impl Visitor for DefaultVariant {
//!     type Output = String;
//!     fn visit<G: RegisteredGame>(self) -> String {
//!         format!("{:?}", G::Config::default())
//!     }
//! }
//! assert!(visit("rock-paper-scissors", DefaultVariant).is_some());
//! assert!(visit("no-such-game", DefaultVariant).is_none());
//! ```

#![no_std]
#![forbid(unsafe_code)]

use libretable_conformance::rock_paper_scissors::{self, RockPaperScissors};
use libretable_game_core::Game;

/// A game a front end can host: a [`Game`] whose types may move between
/// threads, as a server hosting matches needs. Implemented for every such
/// game.
pub trait RegisteredGame:
    Game<
        Config: Send + Sync,
        State: Send + Sync,
        Move: Send + Sync,
        View: Send + Sync,
        Event: Send + Sync,
    > + Send
    + Sync
    + 'static
{
}

impl<G> RegisteredGame for G where
    G: Game<
            Config: Send + Sync,
            State: Send + Sync,
            Move: Send + Sync,
            View: Send + Sync,
            Event: Send + Sync,
        > + Send
        + Sync
        + 'static
{
}

/// Generic code to run for a game picked at run time.
pub trait Visitor {
    /// What the visit returns.
    type Output;

    /// Runs with the game's type.
    fn visit<G: RegisteredGame>(self) -> Self::Output;
}

/// One registered game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The game's ID, e.g. `"rock-paper-scissors"`.
    pub id: &'static str,
    /// Every message ID the game can emit, its ID included (the game's
    /// `MESSAGE_IDS`, see docs/design/localization.md).
    pub message_ids: &'static [&'static str],
}

macro_rules! games {
    ($($game:ty => $message_ids:expr),* $(,)?) => {
        /// Every registered game, sorted by ID.
        pub const GAMES: &[Entry] = &[$(Entry {
            id: <$game as Game>::ID.as_str(),
            message_ids: $message_ids,
        }),*];

        /// Runs `visitor` with the game whose ID is `id`; `None` if no game
        /// has that ID.
        pub fn visit<V: Visitor>(id: &str, visitor: V) -> Option<V::Output> {
            $(
                if id == <$game as Game>::ID.as_str() {
                    return Some(visitor.visit::<$game>());
                }
            )*
            None
        }
    };
}

games! {
    RockPaperScissors => rock_paper_scissors::MESSAGE_IDS,
}

/// The entry of the game with `id`.
#[must_use]
pub fn find(id: &str) -> Option<&'static Entry> {
    GAMES.iter().find(|entry| entry.id == id)
}
