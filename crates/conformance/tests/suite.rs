//! The conformance harness: green for rock-paper-scissors, and each check
//! catches a game that breaks it.

use libretable_conformance::rock_paper_scissors::{self, RockPaperScissors as Rps, Shape};
use libretable_conformance::{Check, Failure, Suite, run};
use libretable_game_core::{
    Choice, Game, GameId, GameRng, IllegalMove, InvalidVariant, Layout, Observer, Outcome, Pending,
    Seat, SeatCount, SeatResult, SeatSet, Seed, TableModel, Variant, Visibility, Zone, ZoneId,
    ZoneKind,
};
use serde::{Deserialize, Serialize};

#[test]
fn rock_paper_scissors_passes() {
    let report = run(&rock_paper_scissors::suite(100, Seed::from_u64(1))).unwrap();
    assert_eq!(report.matches, 6 * 100);
    // seats × rounds moves per match: (2 + 3 + 7) × (1 + 3) per 100 matches.
    assert_eq!(report.moves, 100 * 12 * 4);
    assert_eq!(report.chance_steps, 0);
    assert!(report.leak_variants > 0);
}

#[test]
fn a_run_is_reproducible() {
    let suite = rock_paper_scissors::suite(3, Seed::from_u64(7));
    assert_eq!(run(&suite), run(&suite));
}

#[test]
fn a_too_small_step_bound_fails_termination() {
    let mut suite = rock_paper_scissors::suite(1, Seed::from_u64(1));
    suite.step_bound = |config, seats| usize::from(seats.get()) * usize::from(config.rounds) - 1;
    assert_eq!(check(run(&suite)), Check::Termination);
}

#[test]
fn a_visible_difference_fails_the_leak_test() {
    let mut suite = rock_paper_scissors::suite(1, Seed::from_u64(1));
    // A wrong declaration: the seat's own throw is not hidden from it, so a
    // correct view must show the difference.
    suite.hidden_variants = Some(|state, seat, _| {
        let mut state = state.clone();
        let mut rng = GameRng::from_seed(Seed::from_u64(0));
        match Rps::apply(&mut state, seat, &Shape::Rock, &mut rng) {
            Ok(_) => vec![state],
            Err(_) => Vec::new(),
        }
    });
    assert_eq!(check(run(&suite)), Check::Leak);
}

#[test]
fn an_invalid_setup_fails() {
    let mut suite = rock_paper_scissors::suite(1, Seed::from_u64(1));
    suite.setups = vec![(rock_paper_scissors::Config { rounds: 0 }, seats(2))];
    assert_eq!(check(run(&suite)), Check::Setup);
}

// --- A small game that can be told to break one property ---

/// Two seats alternately add 1 or 2 to a counter; the game ends at `target`.
struct Counter;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Config {
    target: u8,
    bug: Option<Bug>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Bug {
    /// Lists 3 as legal but rejects it.
    RejectsLegal,
    /// Accepts 3 although it is not listed.
    AcceptsUnlisted,
    /// Changes the state while rejecting a move.
    DirtyReject,
    /// Pending with an empty seat set.
    EmptyPending,
    /// Lists moves for the seat whose turn it is not.
    OffTurnMoves,
    /// Never finishes: moves subtract instead of add once at the target.
    NeverEnds,
    /// Uses the generator on setup in a way replay does not repeat.
    Nondeterministic,
    /// The state drops a field when serialized.
    LossySerde,
    /// Outcome for one seat only.
    ShortOutcome,
    /// Every move has the same (empty) choice path.
    SameChoices,
    /// The table has two zones with the same ID.
    DuplicateZone,
}

impl Variant for Config {
    fn validate(&self, seats: SeatCount) -> Result<(), InvalidVariant> {
        if seats.get() == 2 {
            Ok(())
        } else {
            Err(InvalidVariant {
                reason: "counter-two-seats",
            })
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct State {
    config: Config,
    total: u8,
    turn: u8,
    noise: u8,
    /// Not serialized; non-zero only with `Bug::LossySerde`.
    #[serde(skip)]
    cache: u8,
}

static SETUPS: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

impl Counter {
    fn bug(state: &State, bug: Bug) -> bool {
        state.config.bug == Some(bug)
    }
}

impl Game for Counter {
    type Config = Config;
    type State = State;
    type Move = u8;
    type View = State;
    type Event = u8;

    const ID: GameId = GameId::new("counter");

    fn setup(config: &Config, _seats: SeatCount, _rng: &mut GameRng) -> State {
        let noise = if config.bug == Some(Bug::Nondeterministic) {
            // Global state: exactly what games must not use.
            SETUPS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 2
        } else {
            0
        };
        State {
            config: config.clone(),
            total: 0,
            turn: 0,
            noise,
            cache: u8::from(config.bug == Some(Bug::LossySerde)),
        }
    }

    fn pending(state: &State) -> Pending {
        if state.total >= state.config.target && !Self::bug(state, Bug::NeverEnds) {
            Pending::Finished
        } else if Self::bug(state, Bug::EmptyPending) {
            Pending::All(SeatSet::EMPTY)
        } else {
            Pending::All(SeatSet::single(seat(state.turn)))
        }
    }

    fn legal_moves(state: &State, s: Seat) -> Vec<u8> {
        let mut moves = vec![1, 2];
        if Self::bug(state, Bug::RejectsLegal) {
            moves.push(3);
        }
        if Self::pending(state).allows(s)
            || (Self::bug(state, Bug::OffTurnMoves) && Self::pending(state) != Pending::Finished)
        {
            moves
        } else {
            Vec::new()
        }
    }

    fn apply(
        state: &mut State,
        s: Seat,
        mv: &u8,
        _rng: &mut GameRng,
    ) -> Result<Vec<u8>, IllegalMove> {
        let accepted = matches!(mv, 1 | 2) || (*mv == 3 && Self::bug(state, Bug::AcceptsUnlisted));
        if !Self::pending(state).allows(s) || !accepted {
            if Self::bug(state, Bug::DirtyReject) {
                state.noise = state.noise.wrapping_add(1);
            }
            return Err(IllegalMove {
                reason: "counter-illegal",
            });
        }
        state.total = state.total.saturating_add(*mv);
        state.turn = 1 - state.turn;
        Ok(vec![*mv])
    }

    fn view(state: &State, _observer: Observer) -> State {
        state.clone()
    }

    fn table(view: &State) -> TableModel {
        let zone = || Zone {
            id: ZoneId(0),
            kind: ZoneKind::Other,
            label: "counter-zone".into(),
            owner: None,
            visibility: Visibility::Public,
            layout: Layout::Row,
            components: Vec::new(),
        };
        let zones = if Self::bug(view, Bug::DuplicateZone) {
            vec![zone(), zone()]
        } else {
            vec![zone()]
        };
        TableModel {
            zones,
            indicators: Vec::new(),
        }
    }

    fn choices(view: &State, mv: &u8) -> Vec<Choice> {
        if Self::bug(view, Bug::SameChoices) {
            Vec::new()
        } else {
            vec![Choice::Number(i64::from(*mv))]
        }
    }

    fn outcome(state: &State) -> Option<Outcome> {
        if Self::pending(state) != Pending::Finished {
            return None;
        }
        // The seat that reached the target wins.
        let winner = 1 - state.turn;
        let n = if Self::bug(state, Bug::ShortOutcome) {
            1
        } else {
            2
        };
        Some(Outcome {
            seats: (0..n)
                .map(|s| SeatResult {
                    rank: if s == winner { 1 } else { 2 },
                    score: 0,
                })
                .collect(),
            teams: Vec::new(),
        })
    }
}

fn seat(index: u8) -> Seat {
    Seat::new(index).unwrap()
}

fn seats(n: u8) -> SeatCount {
    SeatCount::new(n).unwrap()
}

fn counter_suite(bug: Option<Bug>) -> Suite<Counter> {
    Suite {
        setups: vec![(Config { target: 9, bug }, seats(2))],
        matches: 5,
        seed: Seed::from_u64(3),
        step_bound: |config, _| usize::from(config.target),
        candidate_moves: |_| vec![0, 1, 2, 3],
        hidden_variants: None,
        message_ids: &[
            "counter",
            "counter-two-seats",
            "counter-illegal",
            "counter-zone",
        ],
        determinize: None,
    }
}

fn check<T: std::fmt::Debug>(result: Result<T, Failure>) -> Check {
    let failure = result.unwrap_err();
    // The record reproduces the failure and is valid JSON.
    assert!(failure.record.starts_with('{'), "{failure}");
    failure.check
}

#[test]
fn a_correct_game_passes() {
    let report = run(&counter_suite(None)).unwrap();
    assert_eq!(report.matches, 5);
}

#[test]
fn each_bug_is_caught_by_its_check() {
    for (bug, expected) in [
        (Bug::RejectsLegal, Check::Legality),
        (Bug::AcceptsUnlisted, Check::Legality),
        (Bug::DirtyReject, Check::Legality),
        (Bug::EmptyPending, Check::Legality),
        (Bug::OffTurnMoves, Check::Legality),
        (Bug::NeverEnds, Check::Termination),
        (Bug::Nondeterministic, Check::Determinism),
        (Bug::LossySerde, Check::RoundTrip),
        (Bug::ShortOutcome, Check::Legality),
        (Bug::SameChoices, Check::Selection),
        (Bug::DuplicateZone, Check::Table),
    ] {
        assert_eq!(check(run(&counter_suite(Some(bug)))), expected, "{bug:?}");
    }
}

#[test]
fn an_undeclared_zone_label_fails_messages() {
    let mut suite = counter_suite(None);
    suite.message_ids = &["counter", "counter-illegal"];
    assert_eq!(check(run(&suite)), Check::Messages);
}

#[test]
fn an_undeclared_rejection_reason_fails_messages() {
    let mut suite = counter_suite(None);
    suite.message_ids = &["counter", "counter-zone"];
    assert_eq!(check(run(&suite)), Check::Messages);
}

#[test]
fn an_undeclared_option_fails_messages() {
    let mut suite = rock_paper_scissors::suite(1, Seed::from_u64(1));
    suite.message_ids = &[
        "rock-paper-scissors",
        "rps-not-pending",
        "rps-zone-throw",
        "rps-zone-last-round",
        "rps-indicator-round",
        "rps-indicator-score",
    ];
    let failure = run(&suite).unwrap_err();
    assert_eq!(failure.check, Check::Messages);
    assert!(failure.message.contains("option choice"), "{failure}");
}

#[test]
fn a_malformed_message_list_fails_before_any_match() {
    for ids in [
        &["counter-zone", "counter-illegal"][..],
        &["counter", "Counter-Zone"][..],
        &["counter", "counter--zone"][..],
        &["counter", "counter", "counter-zone"][..],
    ] {
        let mut suite = counter_suite(None);
        suite.message_ids = ids;
        let failure = run(&suite).unwrap_err();
        assert_eq!(failure.check, Check::Messages, "{ids:?}");
        assert!(failure.record.is_empty(), "{ids:?}");
    }
}

#[test]
fn a_determinizer_that_ignores_the_view_is_caught() {
    let mut suite = rock_paper_scissors::suite(5, Seed::from_u64(1));
    suite.determinize = Some(|_, _, rng| Rps::setup(&Default::default(), seats(2), rng));
    assert_eq!(check(run(&suite)), Check::Determinize);
}
