//! The conformance suite for Mau-Mau, over the standard ruleset and every
//! house rule.

use libretable_conformance::{Suite, run};
use libretable_game_core::{GameRng, Seat, SeatCount, Seed};
use libretable_mau_mau::{
    Announce, Config, MESSAGE_IDS, MauMau, Scoring, State, TURN_CAP_PER_SEAT, candidate_moves,
    hidden_variants,
};

fn seats(n: u8) -> SeatCount {
    SeatCount::new(n).unwrap()
}

/// Every house rule switched away from the standard ruleset.
fn house_rules() -> Config {
    Config {
        seven_stacking: false,
        eight_counter: true,
        jack_on_jack: true,
        jack_finish: false,
        ace_again: true,
        nine_reverses: true,
        announce: Announce::Manual,
        mau_penalty: 1,
        rounds: 2,
        scoring: Scoring::CardCount,
        ..Config::default()
    }
}

fn suite(setups: Vec<(Config, SeatCount)>, matches: usize, seed: u64) -> Suite<MauMau> {
    Suite {
        setups,
        matches,
        seed: Seed::from_u64(seed),
        // At most four moves per turn (call, catch, draw, play or keep).
        step_bound: |config, seats| {
            let turns = TURN_CAP_PER_SEAT as usize * usize::from(seats.get());
            usize::from(config.rounds) * (turns * 4 + 4)
        },
        candidate_moves: |state: &State| candidate_moves(state.config()),
        hidden_variants: Some(|state: &State, seat: Seat, rng: &mut GameRng| {
            hidden_variants(state, seat, rng, 2)
        }),
        message_ids: MESSAGE_IDS,
    }
}

#[test]
fn standard_rules() {
    let setups = [2, 3, 5]
        .into_iter()
        .map(|n| (Config::default(), seats(n)))
        .collect();
    let report = run(&suite(setups, 12, 1)).unwrap_or_else(|failure| panic!("{failure}"));
    assert!(report.leak_variants > 0);
}

#[test]
fn every_house_rule() {
    let setups = vec![(house_rules(), seats(2)), (house_rules(), seats(4))];
    run(&suite(setups, 8, 2)).unwrap_or_else(|failure| panic!("{failure}"));
}

#[test]
fn manual_announcements_with_standard_cards() {
    let manual = Config {
        announce: Announce::Manual,
        rounds: 3,
        ..Config::default()
    };
    let setups = vec![(manual.clone(), seats(2)), (manual, seats(3))];
    run(&suite(setups, 8, 3)).unwrap_or_else(|failure| panic!("{failure}"));
}

#[test]
fn reshuffles_and_two_decks() {
    let big_hands = Config {
        hand_size: 7,
        ..Config::default()
    };
    let two_decks = Config {
        decks: 2,
        ..house_rules()
    };
    let setups = vec![(big_hands, seats(3)), (two_decks, seats(7))];
    run(&suite(setups, 4, 4)).unwrap_or_else(|failure| panic!("{failure}"));
}
