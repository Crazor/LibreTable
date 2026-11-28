//! Games the CLI knows, by ID.
//!
//! The commands are generic over [`Game`]; this module erases the type so a
//! game can be picked by its ID at run time. The games themselves are listed
//! in `libretable-registry`, shared with the server.

use std::marker::PhantomData;

use libretable_game_core::{Determinize, Game};
use libretable_registry::{GAMES, RegisteredGame, Visitor};

use crate::commands::{self, BotMaker, PlayOptions, ReplayOptions, SimulateOptions};
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

/// One game's commands, with the bots it supports.
struct Entry<G: Game> {
    bots: BotMaker<G>,
    game: PhantomData<fn() -> G>,
}

impl<G: Game> GameEntry for Entry<G> {
    fn id(&self) -> &'static str {
        G::ID.as_str()
    }

    fn play(&self, options: &PlayOptions, io: &mut Io<'_>) -> Result<(), CliError> {
        commands::play::<G>(options, self.bots, io)
    }

    fn replay(&self, json: &str, options: &ReplayOptions, io: &mut Io<'_>) -> Result<(), CliError> {
        commands::replay::<G>(json, options, io)
    }

    fn simulate(&self, options: &SimulateOptions, io: &mut Io<'_>) -> Result<(), CliError> {
        commands::simulate::<G>(options, self.bots, io)
    }
}

struct MakeEntry;

impl Visitor for MakeEntry {
    type Output = Box<dyn GameEntry>;

    fn visit<G: RegisteredGame>(self) -> Box<dyn GameEntry> {
        Box::new(Entry::<G> {
            bots: commands::random_bots::<G>,
            game: PhantomData,
        })
    }

    fn visit_searchable<G: RegisteredGame + Determinize>(self) -> Box<dyn GameEntry> {
        Box::new(Entry::<G> {
            bots: commands::search_bots::<G>,
            game: PhantomData,
        })
    }
}

/// Every registered game, sorted by ID.
#[must_use]
pub fn games() -> Vec<Box<dyn GameEntry>> {
    GAMES
        .iter()
        .filter_map(|entry| libretable_registry::visit(entry.id, MakeEntry))
        .collect()
}

/// The game with `id`.
#[must_use]
pub fn find(id: &str) -> Option<Box<dyn GameEntry>> {
    libretable_registry::visit(id, MakeEntry)
}
