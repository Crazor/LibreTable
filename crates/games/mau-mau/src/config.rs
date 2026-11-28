//! The variant: standard ruleset plus typed house-rule options.

use libretable_game_core::{InvalidVariant, SeatCount, Variant};
use serde::{Deserialize, Serialize};

/// Cards in one deck.
pub const DECK_SIZE: u8 = 32;

/// Who announces the last card.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Announce {
    /// The game announces it; nobody can forget.
    #[default]
    Auto,
    /// Players call it themselves; the next player may catch a missed or
    /// false call.
    Manual,
}

/// How rounds are scored.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scoring {
    /// A round's winner gets one round win.
    #[default]
    WinLose,
    /// Each card left in hand is one penalty point.
    CardCount,
}

/// The variant. `Default` is the standard ruleset; in JSON every field is
/// optional and defaults to the standard ruleset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)] // Each bool is one house rule.
pub struct Config {
    /// Number of 32-card decks, 1 or 2.
    pub decks: u8,
    /// Cards dealt to each seat, 3 to 7.
    pub hand_size: u8,
    /// A 7 may counter an open draw-two penalty.
    pub seven_stacking: bool,
    /// An 8 may pass a skip on.
    pub eight_counter: bool,
    /// A Jack may be played on a Jack.
    pub jack_on_jack: bool,
    /// A Jack may be the last card.
    pub jack_finish: bool,
    /// After an Ace the same player moves again.
    pub ace_again: bool,
    /// A 9 reverses the direction of play.
    pub nine_reverses: bool,
    /// Who announces the last card.
    pub announce: Announce,
    /// Cards drawn for a caught missed or false call, or a false catch: 1 or 2.
    pub mau_penalty: u8,
    /// Rounds in a match, 1 to 20.
    pub rounds: u8,
    /// How rounds are scored.
    pub scoring: Scoring,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            decks: 1,
            hand_size: 5,
            seven_stacking: true,
            eight_counter: false,
            jack_on_jack: false,
            jack_finish: true,
            ace_again: false,
            nine_reverses: false,
            announce: Announce::Auto,
            mau_penalty: 2,
            rounds: 1,
            scoring: Scoring::WinLose,
        }
    }
}

impl Config {
    /// Cards in play: 32 per deck.
    #[must_use]
    pub const fn cards(&self) -> u8 {
        DECK_SIZE * self.decks
    }

    /// Whether players call the last card themselves.
    #[must_use]
    pub fn manual(&self) -> bool {
        self.announce == Announce::Manual
    }
}

const fn invalid(reason: &'static str) -> Result<(), InvalidVariant> {
    Err(InvalidVariant { reason })
}

impl Variant for Config {
    fn validate(&self, seats: SeatCount) -> Result<(), InvalidVariant> {
        if !(1..=2).contains(&self.decks) {
            return invalid("mau-mau-invalid-decks");
        }
        if !(3..=7).contains(&self.hand_size) {
            return invalid("mau-mau-invalid-hand-size");
        }
        if !(1..=2).contains(&self.mau_penalty) {
            return invalid("mau-mau-invalid-penalty");
        }
        if !(1..=20).contains(&self.rounds) {
            return invalid("mau-mau-invalid-rounds");
        }
        let seats = u16::from(seats.get());
        if seats < 2 {
            return invalid("mau-mau-too-few-seats");
        }
        if seats > 5 * u16::from(self.decks) {
            return invalid("mau-mau-too-many-seats");
        }
        // Every hand, the start card and one card per seat in the draw pile.
        if seats * u16::from(self.hand_size) + 1 + seats > u16::from(self.cards()) {
            return invalid("mau-mau-deck-too-small");
        }
        Ok(())
    }
}
