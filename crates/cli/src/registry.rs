//! Games the CLI knows, by ID.
//!
//! The commands are generic over [`Game`]; the registry erases the type so a
//! game can be picked by its ID at run time. Adding a game is one line in
//! [`games`].

use std::marker::PhantomData;

use libretable_conformance::rock_paper_scissors::RockPaperScissors;
use libretable_game_core::Game;

use crate::commands::{self, PlayOptions, ReplayOptions, SimulateOptions};
use crate::{CliError, Io};

/// The commands of one game, with the game type erased.
pub trait GameEntry {
    /// The game's ID, e.g. `"rock-paper-scissors"`.
    fn id(&self) -> &'static str;

    /// Plays a match in the terminal.
    ///
    /// # Errors
    ///
    /// [`CliError`] on invalid options or I/O failure.
    fn play(&self, options: &PlayOptions, io: &mut Io<'_>) -> Result<(), CliError>;

    /// Replays a record given as JSON text.
    ///
    /// # Errors
    ///
    /// [`CliError`] if the record does not parse or replay.
    fn replay(&self, json: &str, options: &ReplayOptions, io: &mut Io<'_>) -> Result<(), CliError>;

    /// Simulates bot-only matches and prints a summary.
    ///
    /// # Errors
    ///
    /// [`CliError`] on invalid options or a match that does not finish.
    fn simulate(&self, options: &SimulateOptions, io: &mut Io<'_>) -> Result<(), CliError>;
}

struct Entry<G>(PhantomData<fn() -> G>);

impl<G: Game> GameEntry for Entry<G> {
    fn id(&self) -> &'static str {
        G::ID.as_str()
    }

    fn play(&self, options: &PlayOptions, io: &mut Io<'_>) -> Result<(), CliError> {
        commands::play::<G>(options, io)
    }

    fn replay(&self, json: &str, options: &ReplayOptions, io: &mut Io<'_>) -> Result<(), CliError> {
        commands::replay::<G>(json, options, io)
    }

    fn simulate(&self, options: &SimulateOptions, io: &mut Io<'_>) -> Result<(), CliError> {
        commands::simulate::<G>(options, io)
    }
}

fn entry<G: Game + 'static>() -> Box<dyn GameEntry> {
    Box::new(Entry::<G>(PhantomData))
}

/// Every registered game, sorted by ID.
#[must_use]
pub fn games() -> Vec<Box<dyn GameEntry>> {
    vec![entry::<RockPaperScissors>()]
}

/// The game with `id`.
#[must_use]
pub fn find(id: &str) -> Option<Box<dyn GameEntry>> {
    games().into_iter().find(|game| game.id() == id)
}
