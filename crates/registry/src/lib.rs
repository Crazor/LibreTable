//! The games front ends know, by ID.
//!
//! Front ends (the CLI, the server, later the bindings) are generic over
//! [`Game`]; this crate lets them pick a game by its ID at run time. A front
//! end implements [`Visitor`] once and gets its generic code called with the
//! game's type: [`visit`] for one game, [`GAMES`] for the list.
//!
//! Adding a game is one line in the `games!` invocation below, marked
//! `searchable` if it implements `Determinize` (search bots can play it) or
//! `plain` otherwise.
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
use libretable_game_core::{Determinize, Game};
use libretable_mau_mau::{self as mau_mau, MauMau};

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

    /// Runs with the type of a game that also implements [`Determinize`],
    /// so search bots (ISMCTS) can play it. Defaults to
    /// [`visit`](Self::visit).
    fn visit_searchable<G: RegisteredGame + Determinize>(self) -> Self::Output
    where
        Self: Sized,
    {
        self.visit::<G>()
    }
}

/// One registered game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The game's ID, e.g. `"rock-paper-scissors"`.
    pub id: &'static str,
    /// Every message ID the game can emit, its ID included (the game's
    /// `MESSAGE_IDS`, see docs/design/localization.md).
    pub message_ids: &'static [&'static str],
    /// Whether the game implements [`Determinize`], so search bots can play
    /// it.
    pub searchable: bool,
}

macro_rules! games {
    ($($game:ty => $message_ids:expr, $kind:ident);* $(;)?) => {
        /// Every registered game, sorted by ID.
        pub const GAMES: &[Entry] = &[$(Entry {
            id: <$game as Game>::ID.as_str(),
            message_ids: $message_ids,
            searchable: games!(@searchable $kind),
        }),*];

        /// Runs `visitor` with the game whose ID is `id`; `None` if no game
        /// has that ID.
        pub fn visit<V: Visitor>(id: &str, visitor: V) -> Option<V::Output> {
            $(
                if id == <$game as Game>::ID.as_str() {
                    return Some(games!(@visit $kind, $game, visitor));
                }
            )*
            None
        }
    };
    (@searchable searchable) => { true };
    (@searchable plain) => { false };
    (@visit searchable, $game:ty, $visitor:ident) => { $visitor.visit_searchable::<$game>() };
    (@visit plain, $game:ty, $visitor:ident) => { $visitor.visit::<$game>() };
}

// `searchable` games implement `Determinize`; `plain` ones do not.
games! {
    MauMau => mau_mau::MESSAGE_IDS, searchable;
    RockPaperScissors => rock_paper_scissors::MESSAGE_IDS, searchable;
}

/// The entry of the game with `id`.
#[must_use]
pub fn find(id: &str) -> Option<&'static Entry> {
    GAMES.iter().find(|entry| entry.id == id)
}
