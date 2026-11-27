//! Session tests on rock-paper-scissors: selection protocol, rejection,
//! determinism and record round trips.

use libretable_bots::{Player, RandomBot};
use libretable_conformance::rock_paper_scissors::{Config, RockPaperScissors, Shape};
use libretable_game_core::{
    Choice, GameRng, InvalidVariant, Observer, Pending, RecordOf, Seat, SeatCount, SeatSet, Seed,
};
use libretable_session::{Prompt, Session, SubmitError, derive_seed};

type Rps = RockPaperScissors;

fn seat(index: u8) -> Seat {
    Seat::new(index).unwrap()
}

fn new_session(seats: u8, rounds: u8) -> Session<Rps> {
    Session::new(
        Config { rounds },
        SeatCount::new(seats).unwrap(),
        Seed::from_u64(1),
        "test",
    )
    .unwrap()
}

fn rock() -> Choice {
    Choice::Option("rps-rock".into())
}

/// Plays a whole match with one random bot per seat, all derived from
/// `seed`, lowest pending seat first.
fn bot_match(seats: u8, seed: u64) -> Session<Rps> {
    let mut master = GameRng::from_seed(Seed::from_u64(seed));
    let mut session = Session::<Rps>::new(
        Config::default(),
        SeatCount::new(seats).unwrap(),
        derive_seed(&mut master),
        "test",
    )
    .unwrap();
    let mut bots: Vec<RandomBot> = (0..seats)
        .map(|_| RandomBot::new(derive_seed(&mut master)))
        .collect();
    while let Some(next) = session.pending().seats().iter().next() {
        session
            .play(next, &mut bots[usize::from(next.index())])
            .unwrap();
    }
    session
}

#[test]
fn invalid_variant_is_rejected() {
    let error = Session::<Rps>::new(
        Config { rounds: 0 },
        SeatCount::new(2).unwrap(),
        Seed::from_u64(0),
        "test",
    )
    .unwrap_err();
    assert_eq!(
        error,
        InvalidVariant {
            reason: "rps-no-rounds"
        }
    );
}

#[test]
fn prompt_follows_the_selection_protocol() {
    let session = new_session(2, 1);
    assert_eq!(
        session.pending(),
        Pending::All(SeatSet::single(seat(0)).with(seat(1)))
    );
    let options: Vec<Choice> = Shape::ALL
        .iter()
        .map(|shape| Choice::Option(shape.option().into()))
        .collect();
    assert_eq!(
        session.prompt(seat(0), &[]).unwrap(),
        Prompt {
            selectable: options,
            complete: false
        }
    );
    assert_eq!(
        session.prompt(seat(0), &[rock()]).unwrap(),
        Prompt {
            selectable: Vec::new(),
            complete: true
        }
    );
    assert_eq!(
        session.prompt(seat(0), &[Choice::Number(1)]),
        Err(SubmitError::NotSelectable { index: 0 })
    );
    assert_eq!(
        session.prompt(seat(2), &[]),
        Err(SubmitError::NotPending(seat(2)))
    );
}

#[test]
fn rejected_submissions_leave_the_session_unchanged() {
    let mut session = new_session(2, 1);
    let before = session.clone();
    assert_eq!(
        session.submit_choices(seat(0), &[]),
        Err(SubmitError::Incomplete)
    );
    assert_eq!(
        session.submit_choices(seat(0), &[rock(), rock()]),
        Err(SubmitError::NotSelectable { index: 1 })
    );
    assert_eq!(
        session.submit_move(seat(5), Shape::Rock),
        Err(SubmitError::NotPending(seat(5)))
    );
    assert!(session == before);

    session.submit_choices(seat(0), &[rock()]).unwrap();
    let after_first = session.clone();
    // Seat 0 has thrown; it is no longer pending in this round.
    assert_eq!(
        session.submit_choices(seat(0), &[rock()]),
        Err(SubmitError::NotPending(seat(0)))
    );
    assert!(session.decision(seat(0)).is_none());
    assert!(session == after_first);
}

#[test]
fn submit_records_moves_and_finishes() {
    let mut session = new_session(2, 1);
    session.submit_choices(seat(1), &[rock()]).unwrap();
    let events = session
        .submit_choices(seat(0), &[Choice::Option("rps-paper".into())])
        .unwrap();
    assert_eq!(events.len(), 2, "thrown and resolved");
    assert_eq!(session.pending(), Pending::Finished);
    let outcome = session.outcome().unwrap();
    assert_eq!(outcome.result(seat(0)).unwrap().rank, 1);
    let moves: Vec<(u8, Shape)> = session
        .record()
        .moves
        .iter()
        .map(|recorded| (recorded.seat.index(), recorded.mv))
        .collect();
    assert_eq!(moves, [(1, Shape::Rock), (0, Shape::Paper)]);
}

#[test]
fn decision_is_built_from_the_seat_view() {
    let mut session = new_session(3, 1);
    session.submit_choices(seat(1), &[rock()]).unwrap();
    let decision = session.decision(seat(0)).unwrap();
    assert_eq!(decision.seat, seat(0));
    assert_eq!(decision.view, session.view(Observer::Seat(seat(0))));
    // Seat 1's sealed throw is not in seat 0's view.
    assert_eq!(decision.view.throws, [None, None, None]);
    assert_eq!(decision.paths.len(), 3);
    for path in &decision.paths {
        assert!(session.prompt(seat(0), path).unwrap().complete);
    }
}

#[test]
fn bot_matches_are_deterministic() {
    for seed in 0..20 {
        let a = bot_match(3, seed);
        let b = bot_match(3, seed);
        assert!(a == b, "seed {seed}");
        assert_eq!(a.pending(), Pending::Finished);
        assert_eq!(a.record().moves.len(), 9);
    }
    assert_ne!(bot_match(3, 1).record(), bot_match(3, 2).record());
}

#[test]
fn records_round_trip_and_resume() {
    let session = bot_match(4, 7);
    let json = serde_json::to_string(session.record()).unwrap();
    let record: RecordOf<Rps> = serde_json::from_str(&json).unwrap();
    let restored = Session::<Rps>::from_record(record).unwrap();
    assert!(restored == session);

    // A partial record resumes and continues to the same end.
    let mut partial = session.record().clone();
    let rest = partial.moves.split_off(5);
    let mut resumed = Session::<Rps>::from_record(partial).unwrap();
    for recorded in rest {
        resumed.submit_move(recorded.seat, recorded.mv).unwrap();
    }
    assert!(resumed == session);
}

#[test]
fn from_record_rejects_broken_records() {
    let mut record = bot_match(2, 3).into_record();
    // Seat 0 throws twice in the first round.
    assert_eq!(record.moves[0].seat, seat(0));
    record.moves[1].seat = seat(0);
    assert!(Session::<Rps>::from_record(record).is_err());
}

#[test]
fn random_bot_returns_a_legal_path() {
    let session = new_session(2, 1);
    let decision = session.decision(seat(0)).unwrap();
    let mut bot = RandomBot::new(Seed::from_u64(9));
    for _ in 0..50 {
        let path = Player::<Rps>::decide(&mut bot, &decision);
        assert!(decision.paths.contains(&path));
    }
}
