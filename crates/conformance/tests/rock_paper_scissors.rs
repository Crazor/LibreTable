//! Rule tests for the rock-paper-scissors test game, written from the rules in
//! its module documentation.

use Shape::{Paper, Rock, Scissors};
use libretable_conformance::rock_paper_scissors::{
    Config, Event, LAST_ROUND, RockPaperScissors as Rps, Round, Shape, State,
};
use libretable_game_core::{
    Choice, ComponentId, Face, Game, GameRecord, GameRng, IllegalMove, InvalidVariant, MAX_SEATS,
    Observer, Outcome, Pending, RecordOf, ReplayError, Seat, SeatCount, SeatResult, SeatSet, Seed,
    Variant, ZoneId, check_selection, check_table, replay,
};

fn seat(index: u8) -> Seat {
    Seat::new(index).unwrap()
}

fn seats(n: u8) -> SeatCount {
    SeatCount::new(n).unwrap()
}

fn set(indices: &[u8]) -> SeatSet {
    indices.iter().map(|&i| seat(i)).collect()
}

fn rng() -> GameRng {
    GameRng::from_seed(Seed::from_u64(0))
}

fn new_game(n: u8, rounds: u8) -> State {
    Rps::setup(&Config { rounds }, seats(n), &mut rng())
}

/// Plays one round with every seat throwing in seat order.
fn play_round(state: &mut State, throws: &[Shape]) -> Vec<Event> {
    let mut events = Vec::new();
    for (index, shape) in (0..).zip(throws) {
        events.extend(Rps::apply(state, seat(index), shape, &mut rng()).unwrap());
    }
    events
}

fn ranks(outcome: &Outcome) -> Vec<u8> {
    outcome.seats.iter().map(|r| r.rank).collect()
}

fn scores(outcome: &Outcome) -> Vec<i64> {
    outcome.seats.iter().map(|r| r.score).collect()
}

// --- Shapes and round resolution ---

#[test]
fn each_shape_beats_exactly_one_other() {
    assert!(Rock.beats(Scissors));
    assert!(Scissors.beats(Paper));
    assert!(Paper.beats(Rock));
    for a in Shape::ALL {
        assert!(!a.beats(a));
        let beaten = Shape::ALL.into_iter().filter(|&b| a.beats(b)).count();
        assert_eq!(beaten, 1, "{a:?}");
    }
}

#[test]
fn two_seats_every_pairing() {
    for a in Shape::ALL {
        for b in Shape::ALL {
            let round = Round::resolve(vec![a, b]);
            let expected = if a.beats(b) {
                set(&[0])
            } else if b.beats(a) {
                set(&[1])
            } else {
                SeatSet::EMPTY
            };
            assert_eq!(round.winners, expected, "{a:?} vs {b:?}");
            assert_eq!(round.throws, vec![a, b]);
        }
    }
}

#[test]
fn two_shapes_among_many_seats_share_the_win() {
    let round = Round::resolve(vec![Rock, Scissors, Rock, Scissors, Scissors]);
    assert_eq!(round.winners, set(&[0, 2]));
    let round = Round::resolve(vec![Paper, Paper, Scissors]);
    assert_eq!(round.winners, set(&[2]));
}

#[test]
fn all_same_or_all_three_shapes_is_a_draw() {
    assert_eq!(Round::resolve(vec![Paper; 4]).winners, SeatSet::EMPTY);
    assert_eq!(
        Round::resolve(vec![Rock, Paper, Scissors]).winners,
        SeatSet::EMPTY
    );
    assert_eq!(
        Round::resolve(vec![Rock, Rock, Paper, Scissors, Paper]).winners,
        SeatSet::EMPTY
    );
}

// --- Variant ---

#[test]
fn default_is_three_rounds() {
    assert_eq!(Config::default(), Config { rounds: 3 });
}

#[test]
fn variant_validation() {
    let ok = Config::default();
    assert_eq!(
        ok.validate(seats(1)),
        Err(InvalidVariant {
            reason: "rps-too-few-seats"
        })
    );
    assert_eq!(ok.validate(seats(2)), Ok(()));
    assert_eq!(ok.validate(seats(MAX_SEATS)), Ok(()));
    assert_eq!(
        Config { rounds: 0 }.validate(seats(2)),
        Err(InvalidVariant {
            reason: "rps-no-rounds"
        })
    );
    assert_eq!(Config { rounds: 1 }.validate(seats(2)), Ok(()));
}

// --- Simultaneous decisions ---

#[test]
fn every_seat_is_pending_at_the_start() {
    let state = new_game(3, 1);
    assert_eq!(Rps::pending(&state), Pending::All(set(&[0, 1, 2])));
    for i in 0..3 {
        assert_eq!(Rps::legal_moves(&state, seat(i)), Shape::ALL.to_vec());
    }
    assert_eq!(Rps::outcome(&state), None);
}

#[test]
fn a_seat_that_threw_leaves_the_pending_set() {
    let mut state = new_game(3, 1);
    let events = Rps::apply(&mut state, seat(1), &Rock, &mut rng()).unwrap();
    assert_eq!(events, vec![Event::Thrown(seat(1))]);
    assert_eq!(Rps::pending(&state), Pending::All(set(&[0, 2])));
    assert!(Rps::legal_moves(&state, seat(1)).is_empty());
    assert_eq!(Rps::legal_moves(&state, seat(0)), Shape::ALL.to_vec());
}

#[test]
fn throwing_twice_in_a_round_is_rejected_and_changes_nothing() {
    let mut state = new_game(2, 1);
    Rps::apply(&mut state, seat(0), &Rock, &mut rng()).unwrap();
    let before = state.clone();
    assert_eq!(
        Rps::apply(&mut state, seat(0), &Paper, &mut rng()),
        Err(IllegalMove {
            reason: "rps-not-pending"
        })
    );
    assert_eq!(state, before);
}

#[test]
fn seats_outside_the_match_are_rejected() {
    let mut state = new_game(2, 1);
    let before = state.clone();
    assert!(Rps::legal_moves(&state, seat(2)).is_empty());
    assert!(Rps::apply(&mut state, seat(2), &Rock, &mut rng()).is_err());
    assert_eq!(state, before);
}

#[test]
fn arrival_order_does_not_change_the_result() {
    let throws = [Paper, Rock, Rock];
    let mut forward = new_game(3, 1);
    play_round(&mut forward, &throws);
    let mut backward = new_game(3, 1);
    for i in (0..3).rev() {
        Rps::apply(&mut backward, seat(i), &throws[usize::from(i)], &mut rng()).unwrap();
    }
    assert_eq!(forward, backward);
}

#[test]
fn the_last_throw_resolves_the_round_and_opens_the_next() {
    let mut state = new_game(2, 2);
    let events = play_round(&mut state, &[Scissors, Paper]);
    let round = Round {
        throws: vec![Scissors, Paper],
        winners: set(&[0]),
    };
    assert_eq!(
        events,
        vec![
            Event::Thrown(seat(0)),
            Event::Thrown(seat(1)),
            Event::Resolved(round.clone()),
        ]
    );
    assert_eq!(Rps::pending(&state), Pending::All(set(&[0, 1])));
    let view = Rps::view(&state, Observer::Spectator);
    assert_eq!(view.history, vec![round]);
    assert_eq!(view.scores, vec![1, 0]);
    assert_eq!(view.throws, vec![None, None]);
}

// --- End and outcome ---

#[test]
fn draws_count_as_rounds() {
    let mut state = new_game(2, 2);
    play_round(&mut state, &[Rock, Rock]);
    play_round(&mut state, &[Paper, Paper]);
    assert_eq!(Rps::pending(&state), Pending::Finished);
    let outcome = Rps::outcome(&state).unwrap();
    assert_eq!(ranks(&outcome), vec![1, 1]);
    assert_eq!(scores(&outcome), vec![0, 0]);
    assert!(outcome.teams.is_empty());
}

#[test]
fn nothing_is_legal_after_the_end() {
    let mut state = new_game(2, 1);
    play_round(&mut state, &[Rock, Paper]);
    assert_eq!(Rps::pending(&state), Pending::Finished);
    let before = state.clone();
    for i in 0..2 {
        assert!(Rps::legal_moves(&state, seat(i)).is_empty());
        assert!(Rps::apply(&mut state, seat(i), &Rock, &mut rng()).is_err());
    }
    assert_eq!(state, before);
}

#[test]
fn seats_rank_by_rounds_won_and_ties_share() {
    let mut state = new_game(4, 3);
    play_round(&mut state, &[Rock, Scissors, Rock, Scissors]); // 0, 2 win
    play_round(&mut state, &[Paper, Rock, Rock, Rock]); // 0 wins
    play_round(&mut state, &[Rock, Paper, Scissors, Rock]); // draw
    let outcome = Rps::outcome(&state).unwrap();
    assert_eq!(scores(&outcome), vec![2, 0, 1, 0]);
    assert_eq!(ranks(&outcome), vec![1, 3, 2, 3]);
    assert_eq!(
        outcome.result(seat(0)),
        Some(&SeatResult { rank: 1, score: 2 })
    );
}

#[test]
fn shared_first_place_skips_second() {
    let mut state = new_game(3, 1);
    play_round(&mut state, &[Paper, Paper, Rock]);
    let outcome = Rps::outcome(&state).unwrap();
    assert_eq!(ranks(&outcome), vec![1, 1, 3]);
}

#[test]
fn a_match_takes_seats_times_rounds_moves() {
    let mut state = new_game(5, 4);
    let mut moves = 0;
    while let Pending::All(waiting) = Rps::pending(&state) {
        let s = waiting.iter().next().unwrap();
        Rps::apply(&mut state, s, &Rock, &mut rng()).unwrap();
        moves += 1;
    }
    assert_eq!(moves, 20);
    assert_eq!(Rps::pending(&state), Pending::Finished);
}

#[test]
fn the_largest_table_plays() {
    let mut state = new_game(MAX_SEATS, 1);
    let throws: Vec<Shape> = (0..MAX_SEATS)
        .map(|i| if i % 2 == 0 { Rock } else { Scissors })
        .collect();
    play_round(&mut state, &throws);
    let outcome = Rps::outcome(&state).unwrap();
    assert_eq!(outcome.seats.len(), usize::from(MAX_SEATS));
    assert_eq!(outcome.result(seat(62)).unwrap().rank, 1);
    assert_eq!(outcome.result(seat(63)).unwrap().rank, 33);
}

// --- Views ---

#[test]
fn sealed_throws_are_visible_only_to_their_seat() {
    let mut state = new_game(3, 1);
    Rps::apply(&mut state, seat(0), &Paper, &mut rng()).unwrap();
    Rps::apply(&mut state, seat(2), &Scissors, &mut rng()).unwrap();

    let own = Rps::view(&state, Observer::Seat(seat(0)));
    assert_eq!(own.throws, vec![Some(Paper), None, None]);
    assert_eq!(own.waiting, set(&[1]));

    let waiting = Rps::view(&state, Observer::Seat(seat(1)));
    assert_eq!(waiting.throws, vec![None, None, None]);

    let spectator = Rps::view(&state, Observer::Spectator);
    assert_eq!(spectator.throws, vec![None, None, None]);
    assert_eq!(spectator.waiting, set(&[1]));

    let all = Rps::view(&state, Observer::Omniscient);
    assert_eq!(all.throws, vec![Some(Paper), None, Some(Scissors)]);
}

#[test]
fn a_seat_view_does_not_depend_on_other_sealed_throws() {
    for other in Shape::ALL {
        let mut state = new_game(2, 1);
        Rps::apply(&mut state, seat(0), &Rock, &mut rng()).unwrap();
        let reference = Rps::view(&state, Observer::Seat(seat(1)));
        let spectator = Rps::view(&state, Observer::Spectator);
        let mut a = new_game(2, 1);
        Rps::apply(&mut a, seat(0), &other, &mut rng()).unwrap();
        assert_eq!(Rps::view(&a, Observer::Seat(seat(1))), reference);
        assert_eq!(Rps::view(&a, Observer::Spectator), spectator);
    }
}

#[test]
fn thrown_events_do_not_reveal_the_shape() {
    let json = serde_json::to_string(&Event::Thrown(seat(3))).unwrap();
    assert!(!json.contains("rock") && !json.contains("paper") && !json.contains("scissors"));
}

// --- Records and serialization ---

fn sample_record() -> RecordOf<Rps> {
    let mut record = GameRecord::new(
        Rps::ID,
        "test",
        Config { rounds: 2 },
        seats(3),
        Seed::from_u64(9),
    );
    // Arrival order differs from seat order within each round.
    for (s, mv) in [
        (2, Rock),
        (0, Paper),
        (1, Paper),
        (1, Scissors),
        (2, Scissors),
        (0, Rock),
    ] {
        record.push(seat(s), mv);
    }
    record
}

#[test]
fn a_record_replays_to_the_expected_outcome() {
    let (state, _) = replay::<Rps>(&sample_record()).unwrap();
    assert_eq!(Rps::pending(&state), Pending::Finished);
    let outcome = Rps::outcome(&state).unwrap();
    // Round 1: paper beats rock, seats 0 and 1 win. Round 2: rock beats
    // scissors, seat 0 wins.
    assert_eq!(scores(&outcome), vec![2, 1, 0]);
    assert_eq!(ranks(&outcome), vec![1, 2, 3]);
}

#[test]
fn replay_rejects_a_second_throw_in_one_round() {
    let mut record = sample_record();
    record.moves[1].seat = seat(2);
    assert_eq!(
        replay::<Rps>(&record),
        Err(ReplayError::NotPending { index: 1 })
    );
}

#[test]
fn moves_serialize_as_kebab_case_names() {
    assert_eq!(serde_json::to_string(&Scissors).unwrap(), "\"scissors\"");
    let json = serde_json::to_string(&sample_record()).unwrap();
    let back: RecordOf<Rps> = serde_json::from_str(&json).unwrap();
    assert_eq!(back, sample_record());
}

#[test]
fn state_view_and_events_round_trip() {
    let mut state = new_game(3, 2);
    let mut events = play_round(&mut state, &[Rock, Paper, Paper]);
    events.extend(Rps::apply(&mut state, seat(1), &Rock, &mut rng()).unwrap());

    let json = serde_json::to_string(&state).unwrap();
    assert_eq!(serde_json::from_str::<State>(&json).unwrap(), state);
    for observer in [
        Observer::Seat(seat(1)),
        Observer::Spectator,
        Observer::Omniscient,
    ] {
        let view = Rps::view(&state, observer);
        let json = serde_json::to_string(&view).unwrap();
        assert_eq!(serde_json::from_str::<_>(&json).ok(), Some(view));
    }
    let json = serde_json::to_string(&events).unwrap();
    assert_eq!(serde_json::from_str::<Vec<Event>>(&json).unwrap(), events);
}

// --- Table model and selection ---

#[test]
fn the_selection_check_passes_throughout_a_match() {
    let mut state = new_game(3, 2);
    let throws = [Rock, Paper, Paper, Scissors, Scissors, Scissors];
    for (index, shape) in (0..3).cycle().zip(throws) {
        for s in 0..3 {
            check_selection::<Rps>(&state, seat(s), &rng()).unwrap();
        }
        check_table(&Rps::table(&Rps::view(&state, Observer::Spectator)), None).unwrap();
        Rps::apply(&mut state, seat(index), &shape, &mut rng()).unwrap();
    }
    assert_eq!(Rps::pending(&state), Pending::Finished);
}

#[test]
fn a_throw_is_one_option_choice() {
    let state = new_game(2, 1);
    let view = Rps::view(&state, Observer::Seat(seat(0)));
    assert_eq!(
        Rps::choices(&view, &Scissors),
        vec![Choice::Option("rps-scissors".into())]
    );
}

#[test]
fn the_table_shows_sealed_throws_only_to_their_seat() {
    let mut state = new_game(2, 2);
    Rps::apply(&mut state, seat(0), &Rock, &mut rng()).unwrap();
    let face = |observer, owner: u8| {
        let table = Rps::table(&Rps::view(&state, observer));
        let zone = table.zone(ZoneId(u32::from(owner))).unwrap().clone();
        zone.components.first().map(|c| c.face.clone())
    };
    assert!(
        matches!(face(Observer::Seat(seat(0)), 0), Some(Face::Shown { key, .. }) if key == "rps-rock")
    );
    assert_eq!(face(Observer::Seat(seat(1)), 0), Some(Face::Hidden));
    assert_eq!(face(Observer::Spectator, 0), Some(Face::Hidden));
    // Seat 1 has not thrown, so its board is empty.
    assert_eq!(face(Observer::Seat(seat(0)), 1), None);

    Rps::apply(&mut state, seat(1), &Paper, &mut rng()).unwrap();
    let table = Rps::table(&Rps::view(&state, Observer::Spectator));
    let last = table.zone(LAST_ROUND).unwrap();
    assert_eq!(
        last.components.iter().map(|c| c.id).collect::<Vec<_>>(),
        [ComponentId(64), ComponentId(65)]
    );
    assert!(last.components.iter().all(|c| c.face != Face::Hidden));
}
