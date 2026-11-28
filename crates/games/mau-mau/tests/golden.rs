//! Golden replays: the example games of docs/games/mau-mau/rules.md.
//!
//! Each record in `tests/golden/` replays to a finished match with a pinned
//! outcome, and its moves show the rules its example is about. A rule change
//! that alters a replay re-records it in the same change, with a note why:
//!
//! ```text
//! MAU_MAU_RECORD_GOLDEN=1 cargo test -p libretable-mau-mau --test golden -- --ignored
//! ```
//!
//! Recording searches seeds and plays a weighted random policy until a match
//! shows every feature of its example.

use std::collections::BTreeSet;

use libretable_cards::Rank;
use libretable_game_core::{
    Game, GameRecord, GameRng, Observer, Outcome, Pending, RecordOf, Seat, SeatCount, Seed, replay,
};
use libretable_mau_mau::{Announce, Config, Event, MauMau, Move, Scoring, State, card};

/// A rule an example game must show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Feature {
    DrawThenPlay,
    CounteredSeven,
    DrewFour,
    JackWish,
    Winner,
    Reshuffle,
    Skip,
    WishThroughDraw,
    MissedCallCaught,
    FalseCallCaught,
    FalseCatch,
    AceAgain,
    Reversal,
}

use Feature::*;

struct Example {
    name: &'static str,
    config: Config,
    seats: u8,
    features: &'static [Feature],
    record: &'static str,
    outcome: &'static str,
}

fn examples() -> Vec<Example> {
    vec![
        Example {
            name: "two-players-standard",
            config: Config::default(),
            seats: 2,
            features: &[DrawThenPlay, CounteredSeven, DrewFour, JackWish, Winner],
            record: include_str!("golden/two-players-standard.json"),
            outcome: include_str!("golden/two-players-standard.outcome.json"),
        },
        Example {
            name: "three-players-reshuffle",
            config: Config {
                hand_size: 7,
                ..Config::default()
            },
            seats: 3,
            features: &[Reshuffle, Skip, WishThroughDraw, Winner],
            record: include_str!("golden/three-players-reshuffle.json"),
            outcome: include_str!("golden/three-players-reshuffle.outcome.json"),
        },
        Example {
            name: "four-players-house-rules",
            config: Config {
                announce: Announce::Manual,
                ace_again: true,
                nine_reverses: true,
                scoring: Scoring::CardCount,
                rounds: 2,
                ..Config::default()
            },
            seats: 4,
            features: &[
                MissedCallCaught,
                FalseCallCaught,
                FalseCatch,
                AceAgain,
                Reversal,
            ],
            record: include_str!("golden/four-players-house-rules.json"),
            outcome: include_str!("golden/four-players-house-rules.outcome.json"),
        },
    ]
}

fn pending_seat(state: &State) -> Option<Seat> {
    match MauMau::pending(state) {
        Pending::All(set) => set.iter().next(),
        _ => None,
    }
}

/// The features one move shows, judged from the public view before it, the
/// move, its events and who is pending after it.
fn features_of(
    before: &State,
    seat: Seat,
    mv: Move,
    events: &[Event],
    after: &State,
) -> Vec<Feature> {
    let view = MauMau::view(before, Observer::Spectator);
    let mut found = Vec::new();
    match mv {
        Move::Play { card: c, wish } => {
            let rank = card(c).rank;
            if rank == Rank::Seven && view.penalty > 0 {
                found.push(CounteredSeven);
            }
            if wish.is_some() {
                found.push(JackWish);
            }
            if rank == Rank::Ace && before.config().ace_again && pending_seat(after) == Some(seat) {
                found.push(AceAgain);
            }
            if view.phase == libretable_mau_mau::Phase::Drawn {
                found.push(DrawThenPlay);
            }
        }
        Move::Draw if view.wish.is_some() => found.push(WishThroughDraw),
        Move::Catch => {
            for event in events {
                if let Event::Caught {
                    correct, penalized, ..
                } = event
                {
                    found.push(match (correct, view.last_card.contains(*penalized)) {
                        (false, _) => FalseCatch,
                        (true, true) => FalseCallCaught,
                        (true, false) => MissedCallCaught,
                    });
                }
            }
        }
        _ => {}
    }
    for event in events {
        match event {
            Event::Drew { count: 4, .. } => found.push(DrewFour),
            Event::Reshuffled { .. } => found.push(Reshuffle),
            Event::Skipped(_) => found.push(Skip),
            Event::Reversed => found.push(Reversal),
            Event::RoundEnded(result) if result.winner.is_some() => found.push(Winner),
            _ => {}
        }
    }
    found
}

/// Replays `record` move by move; returns the final state and the features
/// shown.
fn walk(record: &RecordOf<MauMau>) -> (State, BTreeSet<Feature>) {
    let mut rng = GameRng::from_seed(record.seed);
    let mut state = MauMau::setup(&record.variant, record.seats, &mut rng);
    let mut features = BTreeSet::new();
    for recorded in &record.moves {
        assert_eq!(pending_seat(&state), Some(recorded.seat));
        let before = state.clone();
        let events = MauMau::apply(&mut state, recorded.seat, &recorded.mv, &mut rng)
            .unwrap_or_else(|e| panic!("{:?}: {}", recorded.mv, e.reason));
        features.extend(features_of(
            &before,
            recorded.seat,
            recorded.mv,
            &events,
            &state,
        ));
    }
    (state, features)
}

#[test]
fn golden_replays_reach_their_outcome_and_show_their_rules() {
    for example in examples() {
        let record: RecordOf<MauMau> = serde_json::from_str(example.record).unwrap();
        assert_eq!(record.variant, example.config, "{}", example.name);
        let (state, features) = walk(&record);
        assert_eq!(
            MauMau::pending(&state),
            Pending::Finished,
            "{}",
            example.name
        );
        for feature in example.features {
            assert!(
                features.contains(feature),
                "{}: {feature:?} missing",
                example.name
            );
        }
        let outcome: Outcome = serde_json::from_str(example.outcome).unwrap();
        assert_eq!(MauMau::outcome(&state), Some(outcome), "{}", example.name);
        // The reference loop agrees.
        let (replayed, _) = replay::<MauMau>(&record).unwrap();
        assert_eq!(replayed, state, "{}", example.name);
    }
}

/// Weight of a move for the recording policy: mostly play, sometimes call.
fn weight(mv: Move) -> u32 {
    match mv {
        Move::Play { .. } => 6,
        Move::Call | Move::Catch => 2,
        _ => 1,
    }
}

fn record_example(example: &Example) -> (RecordOf<MauMau>, Outcome) {
    let seats = SeatCount::new(example.seats).unwrap();
    for seed in 0..10_000 {
        let mut policy = GameRng::from_seed(Seed::from_u64(1_000_000 + seed));
        let mut record = GameRecord::new(
            MauMau::ID,
            "golden",
            example.config.clone(),
            seats,
            Seed::from_u64(seed),
        );
        let mut rng = GameRng::from_seed(record.seed);
        let mut state = MauMau::setup(&example.config, seats, &mut rng);
        let mut features = BTreeSet::new();
        while let Some(seat) = pending_seat(&state) {
            let moves = MauMau::legal_moves(&state, seat);
            let total: u32 = moves.iter().map(|&m| weight(m)).sum();
            let mut pick = policy.below(total);
            let mv = *moves
                .iter()
                .find(|&&m| {
                    if pick < weight(m) {
                        true
                    } else {
                        pick -= weight(m);
                        false
                    }
                })
                .unwrap();
            let before = state.clone();
            let events = MauMau::apply(&mut state, seat, &mv, &mut rng).unwrap();
            features.extend(features_of(&before, seat, mv, &events, &state));
            record.push(seat, mv);
        }
        if example.features.iter().all(|f| features.contains(f)) && record.moves.len() < 400 {
            return (record, MauMau::outcome(&state).unwrap());
        }
    }
    panic!("no seed shows every feature of {}", example.name);
}

#[test]
#[ignore = "records the golden replays; run with MAU_MAU_RECORD_GOLDEN=1"]
fn record_golden_replays() {
    if std::env::var_os("MAU_MAU_RECORD_GOLDEN").is_none() {
        return;
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    std::fs::create_dir_all(&dir).unwrap();
    for example in examples() {
        let (record, outcome) = record_example(&example);
        let json = serde_json::to_string_pretty(&record).unwrap() + "\n";
        std::fs::write(dir.join(format!("{}.json", example.name)), json).unwrap();
        let json = serde_json::to_string_pretty(&outcome).unwrap() + "\n";
        std::fs::write(dir.join(format!("{}.outcome.json", example.name)), json).unwrap();
    }
}
