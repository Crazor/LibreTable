//! Exercises the `Game` trait and `replay` with a tiny counting game: a chance
//! step picks the starting seat, then seats take turns adding 1 or 2 to a
//! shared total; whoever reaches the target wins.

use libretable_game_core::{
    Choice, Game, GameId, GameRecord, GameRng, IllegalMove, Indicator, IndicatorKind,
    IndicatorValue, InvalidVariant, Observer, Outcome, Pending, RECORD_SCHEMA_VERSION, RecordOf,
    RecordedMove, ReplayError, Seat, SeatCount, SeatResult, SeatSet, Seed, TableModel, Variant,
    check_selection, replay,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Config {
    target: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self { target: 10 }
    }
}

impl Variant for Config {
    fn validate(&self, seats: SeatCount) -> Result<(), InvalidVariant> {
        if !(2..=4).contains(&seats.get()) {
            return Err(InvalidVariant {
                reason: "count-seats",
            });
        }
        if self.target == 0 {
            return Err(InvalidVariant {
                reason: "count-target",
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct State {
    seats: SeatCount,
    target: u8,
    total: u8,
    /// `None` until the chance step picked the starting seat.
    turn: Option<Seat>,
    winner: Option<Seat>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Event {
    Starts(Seat),
    Added { seat: Seat, amount: u8 },
}

struct Count;

impl Game for Count {
    type Config = Config;
    type State = State;
    type Move = u8;
    type View = State;
    type Event = Event;

    const ID: GameId = GameId::new("count-to-n");

    fn setup(config: &Config, seats: SeatCount, _rng: &mut GameRng) -> State {
        State {
            seats,
            target: config.target,
            total: 0,
            turn: None,
            winner: None,
        }
    }

    fn pending(state: &State) -> Pending {
        match (state.winner, state.turn) {
            (Some(_), _) => Pending::Finished,
            (None, None) => Pending::Chance,
            (None, Some(seat)) => Pending::All(SeatSet::single(seat)),
        }
    }

    fn legal_moves(state: &State, seat: Seat) -> Vec<u8> {
        if Self::pending(state).allows(seat) {
            vec![1, 2]
        } else {
            Vec::new()
        }
    }

    fn apply(
        state: &mut State,
        seat: Seat,
        mv: &u8,
        _rng: &mut GameRng,
    ) -> Result<Vec<Event>, IllegalMove> {
        if !Self::legal_moves(state, seat).contains(mv) {
            return Err(IllegalMove {
                reason: "count-illegal",
            });
        }
        state.total += mv;
        if state.total >= state.target {
            state.winner = Some(seat);
        } else {
            let next = (seat.index() + 1) % state.seats.get();
            state.turn = Seat::new(next);
        }
        Ok(vec![Event::Added { seat, amount: *mv }])
    }

    fn apply_chance(state: &mut State, rng: &mut GameRng) -> Vec<Event> {
        let seat = Seat::new(rng.below(u32::from(state.seats.get())) as u8).unwrap();
        state.turn = Some(seat);
        vec![Event::Starts(seat)]
    }

    fn view(state: &State, _observer: Observer) -> State {
        state.clone()
    }

    fn table(view: &State) -> TableModel {
        TableModel {
            zones: Vec::new(),
            indicators: vec![Indicator {
                kind: IndicatorKind::Counter,
                label: "count-total".into(),
                owner: None,
                value: IndicatorValue::Number(i64::from(view.total)),
            }],
        }
    }

    fn choices(_view: &State, mv: &u8) -> Vec<Choice> {
        vec![Choice::Number(i64::from(*mv))]
    }

    fn outcome(state: &State) -> Option<Outcome> {
        let winner = state.winner?;
        let seats = state
            .seats
            .seats()
            .map(|seat| SeatResult {
                rank: if seat == winner { 1 } else { 2 },
                score: 0,
            })
            .collect();
        Some(Outcome {
            seats,
            teams: Vec::new(),
        })
    }
}

fn seats(n: u8) -> SeatCount {
    SeatCount::new(n).unwrap()
}

/// Plays a full game, always adding 2 (or 1 when it exactly finishes).
fn play(seed: u64, n: u8) -> RecordOf<Count> {
    let mut record = GameRecord::new(
        Count::ID,
        "test",
        Config::default(),
        seats(n),
        Seed::from_u64(seed),
    );
    let mut rng = GameRng::from_seed(record.seed);
    let mut state = Count::setup(&record.variant, record.seats, &mut rng);
    loop {
        match Count::pending(&state) {
            Pending::Chance => {
                Count::apply_chance(&mut state, &mut rng);
            }
            Pending::All(set) | Pending::Any(set) => {
                let seat = set.iter().next().unwrap();
                let mv = if state.target - state.total == 1 {
                    1
                } else {
                    2
                };
                Count::apply(&mut state, seat, &mv, &mut rng).unwrap();
                record.push(seat, mv);
            }
            Pending::Finished => return record,
        }
    }
}

#[test]
fn replay_reaches_the_finished_state() {
    let record = play(1, 3);
    let (state, _) = replay::<Count>(&record).unwrap();
    assert_eq!(Count::pending(&state), Pending::Finished);
    assert_eq!(state.total, 10);
    let outcome = Count::outcome(&state).unwrap();
    assert_eq!(outcome.result(state.winner.unwrap()).unwrap().rank, 1);
    assert_eq!(outcome.seats.len(), 3);
}

#[test]
fn replay_is_deterministic() {
    for seed in 0..20 {
        let record = play(seed, 4);
        assert_eq!(replay::<Count>(&record), replay::<Count>(&record));
    }
}

#[test]
fn the_seed_drives_chance() {
    let starters: std::collections::BTreeSet<_> =
        (0..40).map(|seed| play(seed, 4).moves[0].seat).collect();
    assert!(starters.len() > 1, "chance never varied: {starters:?}");
}

#[test]
fn an_empty_record_resolves_initial_chance() {
    let record = GameRecord::new(
        Count::ID,
        "test",
        Config::default(),
        seats(2),
        Seed::from_u64(5),
    );
    let (state, _) = replay::<Count>(&record).unwrap();
    assert!(matches!(Count::pending(&state), Pending::All(_)));
}

#[test]
fn record_json_round_trip() {
    let record = play(7, 2);
    let json = serde_json::to_string(&record).unwrap();
    assert!(json.contains("\"move\":"));
    let back: RecordOf<Count> = serde_json::from_str(&json).unwrap();
    assert_eq!(back, record);
}

#[test]
fn replay_rejects_wrong_game_and_schema() {
    let mut record = play(2, 2);
    record.game = "other".into();
    assert_eq!(replay::<Count>(&record), Err(ReplayError::WrongGame));

    let mut record = play(2, 2);
    record.schema = RECORD_SCHEMA_VERSION + 1;
    assert_eq!(
        replay::<Count>(&record),
        Err(ReplayError::Schema {
            found: RECORD_SCHEMA_VERSION + 1
        })
    );
}

#[test]
fn replay_rejects_invalid_variant() {
    let record = GameRecord::new(
        Count::ID,
        "test",
        Config::default(),
        seats(5),
        Seed::from_u64(0),
    );
    assert_eq!(
        replay::<Count>(&record),
        Err(ReplayError::InvalidVariant(InvalidVariant {
            reason: "count-seats"
        }))
    );
}

#[test]
fn replay_rejects_out_of_turn_and_illegal_moves() {
    let mut record = play(3, 2);
    let first = record.moves[0].seat;
    record.moves[1].seat = first;
    assert_eq!(
        replay::<Count>(&record),
        Err(ReplayError::NotPending { index: 1 })
    );

    let mut record = play(3, 2);
    record.moves[0].mv = 3;
    assert_eq!(
        replay::<Count>(&record),
        Err(ReplayError::IllegalMove {
            index: 0,
            error: IllegalMove {
                reason: "count-illegal"
            }
        })
    );

    // A seat outside the match.
    let mut record = play(3, 2);
    record.moves[0].seat = Seat::new(5).unwrap();
    assert_eq!(
        replay::<Count>(&record),
        Err(ReplayError::NotPending { index: 0 })
    );

    // Moves after the end.
    let mut record = play(3, 2);
    let last = record.moves.last().unwrap().clone();
    record.moves.push(last);
    let index = record.moves.len() - 1;
    assert_eq!(
        replay::<Count>(&record),
        Err(ReplayError::NotPending { index })
    );
}

#[test]
fn replayed_rng_continues_the_stream() {
    let record = play(4, 2);
    let (_, mut a) = replay::<Count>(&record).unwrap();
    let (_, mut b) = replay::<Count>(&record).unwrap();
    assert_eq!(a.next_u64(), b.next_u64());
}

#[test]
fn every_state_passes_the_selection_check() {
    let record = play(5, 3);
    let mut rng = GameRng::from_seed(record.seed);
    let mut state = Count::setup(&record.variant, record.seats, &mut rng);
    for RecordedMove { seat, mv } in &record.moves {
        while Count::pending(&state) == Pending::Chance {
            Count::apply_chance(&mut state, &mut rng);
        }
        for s in record.seats.seats() {
            check_selection::<Count>(&state, s, &rng).unwrap();
        }
        Count::apply(&mut state, *seat, mv, &mut rng).unwrap();
    }
}
