//! The ISMCTS bot: legal answers, determinism, and strength against random
//! play in Mau-Mau.

use libretable_bots::{Decision, IsmctsBot, IsmctsConfig, Player, RandomBot};
use libretable_conformance::rock_paper_scissors::RockPaperScissors;
use libretable_game_core::{Game, GameRng, Observer, Pending, SeatCount, Seed};
use libretable_mau_mau::{Config, MauMau};

/// Plays one match with `players` by seat; returns each seat's rank.
fn play<G: Game>(config: &G::Config, players: &mut [&mut dyn Player<G>], seed: u64) -> Vec<u8> {
    let seats = SeatCount::new(u8::try_from(players.len()).unwrap()).unwrap();
    let mut rng = GameRng::from_seed(Seed::from_u64(seed));
    let mut state = G::setup(config, seats, &mut rng);
    for _ in 0..100_000 {
        let seat = match G::pending(&state) {
            Pending::Finished => break,
            Pending::Chance => {
                G::apply_chance(&mut state, &mut rng);
                continue;
            }
            Pending::All(set) | Pending::Any(set) => set.iter().next().unwrap(),
        };
        let view = G::view(&state, Observer::Seat(seat));
        let moves = G::legal_moves(&state, seat);
        let paths: Vec<_> = moves.iter().map(|mv| G::choices(&view, mv)).collect();
        let decision = Decision { seat, view, paths };
        let path = players[usize::from(seat.index())].decide(&decision);
        let index = decision
            .paths
            .iter()
            .position(|p| *p == path)
            .expect("a legal path");
        G::apply(&mut state, seat, &moves[index], &mut rng).unwrap();
    }
    G::outcome(&state)
        .expect("the match ends")
        .seats
        .iter()
        .map(|result| result.rank)
        .collect()
}

/// One-sided binomial tail `P(X >= wins)` for `X ~ Bin(n, 1/2)`.
fn tail(n: u32, wins: u32) -> f64 {
    let mut total = 0.0;
    for k in wins..=n {
        let mut choose = 1.0;
        for i in 0..k {
            choose *= f64::from(n - i) / f64::from(i + 1);
        }
        total += choose;
    }
    total / 2f64.powi(i32::try_from(n).unwrap())
}

#[test]
fn ismcts_beats_random_play_at_mau_mau() {
    let config = Config::default();
    let search = IsmctsConfig {
        iterations: 150,
        ..IsmctsConfig::default()
    };
    let matches = 40;
    let mut wins = 0;
    for game in 0..matches {
        let mut ismcts = IsmctsBot::with_config(Seed::from_u64(1000 + game), search);
        let mut random = RandomBot::new(Seed::from_u64(2000 + game));
        // Alternate seats, so the starting player has no advantage overall.
        let ismcts_seat = u8::try_from(game % 2).unwrap();
        let ranks = if ismcts_seat == 0 {
            play::<MauMau>(&config, &mut [&mut ismcts, &mut random], game)
        } else {
            play::<MauMau>(&config, &mut [&mut random, &mut ismcts], game)
        };
        // A win needs rank 1 alone; ties and rounds without a winner count
        // as losses.
        let mine = ranks[usize::from(ismcts_seat)];
        let other = ranks[usize::from(1 - ismcts_seat)];
        if mine == 1 && other > 1 {
            wins += 1;
        }
    }
    let p = tail(u32::try_from(matches).unwrap(), wins);
    eprintln!("ISMCTS won {wins} of {matches} (p = {p:.2e})");
    assert!(p < 0.01, "ISMCTS won {wins} of {matches} (p = {p:.4})");
}

#[test]
fn ismcts_answers_with_a_legal_path_and_is_deterministic() {
    let config = Config::default();
    let search = IsmctsConfig {
        iterations: 50,
        ..IsmctsConfig::default()
    };
    let run = || {
        let mut a = IsmctsBot::with_config(Seed::from_u64(1), search);
        let mut b = IsmctsBot::with_config(Seed::from_u64(2), search);
        play::<MauMau>(&config, &mut [&mut a, &mut b], 5)
    };
    assert_eq!(run(), run());
}

#[test]
fn ismcts_plays_simultaneous_games() {
    let config = Default::default();
    let mut a = IsmctsBot::with_config(
        Seed::from_u64(1),
        IsmctsConfig {
            iterations: 30,
            ..IsmctsConfig::default()
        },
    );
    let mut b = RandomBot::new(Seed::from_u64(2));
    let ranks = play::<RockPaperScissors>(&config, &mut [&mut a, &mut b], 1);
    assert_eq!(ranks.len(), 2);
}
