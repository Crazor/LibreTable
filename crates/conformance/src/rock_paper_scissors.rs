//! Rock-paper-scissors: the test game for simultaneous decisions
//! ([`Pending::All`]). It is not shipped.
//!
//! # Rules
//!
//! - **Seats:** 2 to [`MAX_SEATS`].
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
//!
//! # Table and choices
//!
//! Each seat has a player-board zone (owner only) holding its sealed throw as
//! a token, face down for everybody else; a public tableau shows the throws of
//! the last resolved round. Indicators show each seat's score and the round.
//! A throw is one option choice: `rps-rock`, `rps-paper` or `rps-scissors`.

use libretable_game_core::{
    Attribute, Choice, Component, ComponentId, ComponentKind, Determinize, Face, Game, GameId,
    GameRng, IllegalMove, Indicator, IndicatorKind, IndicatorValue, InvalidVariant, Layout,
    MAX_SEATS, Observer, Outcome, Pending, Seat, SeatCount, SeatResult, SeatSet, TableModel,
    Variant, Visibility, Zone, ZoneId, ZoneKind,
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

    /// Internal key, also the asset name: `"rock"`, `"paper"`, `"scissors"`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Rock => "rock",
            Self::Paper => "paper",
            Self::Scissors => "scissors",
        }
    }

    /// Fluent message ID of the option choice that throws this shape.
    #[must_use]
    pub const fn option(self) -> &'static str {
        match self {
            Self::Rock => "rps-rock",
            Self::Paper => "rps-paper",
            Self::Scissors => "rps-scissors",
        }
    }

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

/// Every Fluent message ID this game emits, its game ID first (the ID of its
/// display name). Messages: `locales/<locale>/rock-paper-scissors.ftl`.
pub const MESSAGE_IDS: &[&str] = &[
    "rock-paper-scissors",
    "rps-too-few-seats",
    "rps-no-rounds",
    "rps-not-pending",
    "rps-rock",
    "rps-paper",
    "rps-scissors",
    "rps-zone-throw",
    "rps-zone-last-round",
    "rps-indicator-round",
    "rps-indicator-score",
];

/// Zone of the last resolved round; seat boards are zones `0..seats`.
pub const LAST_ROUND: ZoneId = ZoneId(MAX_SEATS as u32);

fn token(id: u32, shape: Option<Shape>) -> Component {
    Component {
        id: ComponentId(id),
        kind: ComponentKind::Token,
        face: shape.map_or(Face::Hidden, |shape| Face::Shown {
            key: shape.option().into(),
            attributes: vec![Attribute::new("shape", shape.key())],
        }),
        rotation: 0,
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

    /// Seat `n`'s sealed throw is component `n`; the last round's throw of
    /// seat `n` is component `MAX_SEATS + n`.
    fn table(view: &View) -> TableModel {
        let boards = view
            .throws
            .iter()
            .zip((0..).map_while(Seat::new))
            .map(|(throw, owner)| {
                let thrown = !view.waiting.contains(owner);
                Zone {
                    id: ZoneId(u32::from(owner.index())),
                    kind: ZoneKind::PlayerBoard,
                    label: "rps-zone-throw".into(),
                    owner: Some(owner),
                    visibility: Visibility::Owner,
                    layout: Layout::Stack,
                    components: if thrown {
                        vec![token(u32::from(owner.index()), *throw)]
                    } else {
                        Vec::new()
                    },
                }
            });
        let last = view.history.last().map_or_else(Vec::new, |round| {
            round
                .throws
                .iter()
                .zip(u32::from(MAX_SEATS)..)
                .map(|(shape, id)| token(id, Some(*shape)))
                .collect()
        });
        let mut zones: Vec<Zone> = boards.collect();
        zones.push(Zone {
            id: LAST_ROUND,
            kind: ZoneKind::Tableau,
            label: "rps-zone-last-round".into(),
            owner: None,
            visibility: Visibility::Public,
            layout: Layout::Row,
            components: last,
        });
        let round = (view.history.len() + 1).min(usize::from(view.rounds));
        let mut indicators = vec![Indicator {
            kind: IndicatorKind::Counter,
            label: "rps-indicator-round".into(),
            owner: None,
            value: IndicatorValue::Number(round as i64),
        }];
        indicators.extend(view.scores.iter().zip((0..).map_while(Seat::new)).map(
            |(score, owner)| Indicator {
                kind: IndicatorKind::Score,
                label: "rps-indicator-score".into(),
                owner: Some(owner),
                value: IndicatorValue::Number(i64::from(*score)),
            },
        ));
        TableModel { zones, indicators }
    }

    fn choices(_view: &View, mv: &Shape) -> Vec<Choice> {
        vec![Choice::Option(mv.option().into())]
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

/// The conformance suite for this game: 2, 3 and 7 seats with one and three
/// rounds.
#[must_use]
pub fn suite(matches: usize, seed: libretable_game_core::Seed) -> crate::Suite<RockPaperScissors> {
    let setups = [2, 3, 7]
        .into_iter()
        .filter_map(SeatCount::new)
        .flat_map(|seats| [1, 3].map(|rounds| (Config { rounds }, seats)))
        .collect();
    crate::Suite {
        setups,
        matches,
        seed,
        step_bound: |config, seats| usize::from(seats.get()) * usize::from(config.rounds),
        candidate_moves: |_| Shape::ALL.to_vec(),
        hidden_variants: Some(hidden_variants),
        message_ids: MESSAGE_IDS,
        determinize: crate::determinizer::<RockPaperScissors>(),
    }
}

impl Determinize for RockPaperScissors {
    /// The sealed throws of the other seats that have thrown are drawn
    /// uniformly; everything else is in the view.
    fn determinize(view: &View, seat: Seat, rng: &mut GameRng) -> State {
        let throws = view
            .throws
            .iter()
            .zip((0..).map_while(Seat::new))
            .map(|(throw, owner)| {
                if owner == seat || view.waiting.contains(owner) {
                    *throw
                } else {
                    rng.choose(&Shape::ALL).copied()
                }
            })
            .collect();
        State {
            rounds: view.rounds,
            throws,
            history: view.history.clone(),
            scores: view.scores.clone(),
        }
    }
}

/// Up to three states in which some other seat's sealed throw differs: the
/// only information hidden from `seat`.
fn hidden_variants(state: &State, seat: Seat, rng: &mut GameRng) -> Vec<State> {
    let others: Vec<usize> = state
        .throws
        .iter()
        .enumerate()
        .filter(|&(index, throw)| throw.is_some() && index != usize::from(seat.index()))
        .map(|(index, _)| index)
        .collect();
    (0..3)
        .filter_map(|_| {
            let &index = rng.choose(&others)?;
            let mut variant = state.clone();
            let throw = &mut variant.throws[index];
            let others: Vec<Shape> = Shape::ALL
                .into_iter()
                .filter(|shape| Some(*shape) != *throw)
                .collect();
            *throw = rng.choose(&others).copied();
            Some(variant)
        })
        .collect()
}
