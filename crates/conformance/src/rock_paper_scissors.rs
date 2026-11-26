//! Rock-paper-scissors: the test game for simultaneous decisions
//! ([`Pending::All`]). It is not shipped.
//!
//! # Rules
//!
//! - **Seats:** 2 to [`MAX_SEATS`](libretable_game_core::MAX_SEATS).
//! - **Variant:** `rounds`, the number of rounds played, at least 1
//!   (default 3).
//! - **Round:** every seat secretly throws rock, paper or scissors. Rock beats
//!   scissors, scissors beats paper, paper beats rock. Throws are sealed until
//!   the last seat has thrown; then all are revealed at once.
//! - **Round result:** if exactly two different shapes were thrown, every seat
//!   that threw the winning shape wins the round. If all seats threw the same
//!   shape, or all three shapes were thrown, the round is a draw. Every round
//!   counts towards `rounds`, draws included.
//! - **End:** after `rounds` rounds. A seat's score is the number of rounds it
//!   won; seats rank by score, higher is better, equal scores share a rank.
//!
//! There is no chance step and no use of the generator. A match takes exactly
//! `seats × rounds` moves.
//!
//! # Hidden information
//!
//! A seat's view shows its own sealed throw of the current round; other seats
//! and spectators only see who has thrown. Resolved rounds are public.

use libretable_game_core::{
    Game, GameId, GameRng, IllegalMove, InvalidVariant, Observer, Outcome, Pending, Seat,
    SeatCount, SeatResult, SeatSet, Variant,
};
use serde::{Deserialize, Serialize};

/// The variant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// Number of rounds played, at least 1.
    pub rounds: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self { rounds: 3 }
    }
}

impl Variant for Config {
    fn validate(&self, seats: SeatCount) -> Result<(), InvalidVariant> {
        if seats.get() < 2 {
            return Err(InvalidVariant {
                reason: "rps-too-few-seats",
            });
        }
        if self.rounds == 0 {
            return Err(InvalidVariant {
                reason: "rps-no-rounds",
            });
        }
        Ok(())
    }
}

/// A throw; the move of this game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Shape {
    /// Beats scissors.
    Rock,
    /// Beats rock.
    Paper,
    /// Beats paper.
    Scissors,
}

impl Shape {
    /// All shapes, in the order [`legal_moves`](Game::legal_moves) lists them.
    pub const ALL: [Self; 3] = [Self::Rock, Self::Paper, Self::Scissors];

    /// Whether `self` beats `other`.
    #[must_use]
    pub const fn beats(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::Rock, Self::Scissors)
                | (Self::Scissors, Self::Paper)
                | (Self::Paper, Self::Rock)
        )
    }
}

/// A resolved round.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Round {
    /// Every seat's throw, indexed by [`Seat::index`].
    pub throws: Vec<Shape>,
    /// The seats that won the round; empty for a draw.
    pub winners: SeatSet,
}

impl Round {
    /// Resolves a round from every seat's throw.
    #[must_use]
    pub fn resolve(throws: Vec<Shape>) -> Self {
        let present: Vec<Shape> = Shape::ALL
            .into_iter()
            .filter(|shape| throws.contains(shape))
            .collect();
        let winning = match present.as_slice() {
            &[a, b] => Some(if a.beats(b) { a } else { b }),
            _ => None,
        };
        let winners = throws
            .iter()
            .zip(0..)
            .filter(|(shape, _)| Some(**shape) == winning)
            .filter_map(|(_, index)| Seat::new(index))
            .collect();
        Self { throws, winners }
    }
}

/// The full state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    rounds: u8,
    /// Sealed throws of the current round, indexed by seat.
    throws: Vec<Option<Shape>>,
    history: Vec<Round>,
    /// Rounds won, indexed by seat.
    scores: Vec<u8>,
}

/// What one observer may know.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct View {
    /// Number of rounds of the match.
    pub rounds: u8,
    /// Resolved rounds, oldest first.
    pub history: Vec<Round>,
    /// Rounds won, indexed by seat.
    pub scores: Vec<u8>,
    /// Seats that have not thrown yet in the current round.
    pub waiting: SeatSet,
    /// Sealed throws of the current round that the observer may see, indexed
    /// by seat: its own for a seat, none for a spectator.
    pub throws: Vec<Option<Shape>>,
}

/// What happened in one step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    /// A seat threw; the shape stays sealed.
    Thrown(Seat),
    /// The last seat threw and the round was revealed.
    Resolved(Round),
}

/// The game.
#[derive(Clone, Copy, Debug)]
pub struct RockPaperScissors;

impl RockPaperScissors {
    fn waiting(state: &State) -> SeatSet {
        state
            .throws
            .iter()
            .zip(0..)
            .filter(|(throw, _)| throw.is_none())
            .filter_map(|(_, index)| Seat::new(index))
            .collect()
    }
}

impl Game for RockPaperScissors {
    type Config = Config;
    type State = State;
    type Move = Shape;
    type View = View;
    type Event = Event;

    const ID: GameId = GameId::new("rock-paper-scissors");

    fn setup(config: &Config, seats: SeatCount, _rng: &mut GameRng) -> State {
        let n = usize::from(seats.get());
        State {
            rounds: config.rounds,
            throws: vec![None; n],
            history: Vec::new(),
            scores: vec![0; n],
        }
    }

    fn pending(state: &State) -> Pending {
        if state.history.len() == usize::from(state.rounds) {
            Pending::Finished
        } else {
            Pending::All(Self::waiting(state))
        }
    }

    fn legal_moves(state: &State, seat: Seat) -> Vec<Shape> {
        if Self::pending(state).allows(seat) {
            Shape::ALL.to_vec()
        } else {
            Vec::new()
        }
    }

    fn apply(
        state: &mut State,
        seat: Seat,
        mv: &Shape,
        _rng: &mut GameRng,
    ) -> Result<Vec<Event>, IllegalMove> {
        if !Self::pending(state).allows(seat) {
            return Err(IllegalMove {
                reason: "rps-not-pending",
            });
        }
        state.throws[usize::from(seat.index())] = Some(*mv);
        let mut events = vec![Event::Thrown(seat)];
        // Every seat has thrown when no throw is missing.
        let throws: Option<Vec<Shape>> = state.throws.iter().copied().collect();
        if let Some(throws) = throws {
            let round = Round::resolve(throws);
            for winner in round.winners.iter() {
                state.scores[usize::from(winner.index())] += 1;
            }
            state.throws.fill(None);
            state.history.push(round.clone());
            events.push(Event::Resolved(round));
        }
        Ok(events)
    }

    fn view(state: &State, observer: Observer) -> View {
        let throws = state
            .throws
            .iter()
            .zip((0..).map_while(Seat::new))
            .map(|(throw, owner)| throw.filter(|_| observer.sees_private_of(owner)))
            .collect();
        View {
            rounds: state.rounds,
            history: state.history.clone(),
            scores: state.scores.clone(),
            waiting: Self::waiting(state),
            throws,
        }
    }

    fn outcome(state: &State) -> Option<Outcome> {
        if Self::pending(state) != Pending::Finished {
            return None;
        }
        let seats = state
            .scores
            .iter()
            .map(|&score| {
                let better = state.scores.iter().filter(|&&s| s > score).count();
                SeatResult {
                    // At most MAX_SEATS - 1 seats can be better.
                    rank: u8::try_from(better + 1).unwrap_or(u8::MAX),
                    score: i64::from(score),
                }
            })
            .collect();
        Some(Outcome {
            seats,
            teams: Vec::new(),
        })
    }
}
