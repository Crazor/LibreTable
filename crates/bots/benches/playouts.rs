//! Playout benchmark (architecture: bots, performance): random playouts and
//! ISMCTS iterations per second, per game.
//!
//! ```text
//! cargo bench -p libretable-bots
//! ```
//!
//! Plain `std::time` timing, no benchmark framework; compare numbers from
//! the same machine only.

use std::time::{Duration, Instant};

use libretable_bots::{Decision, IsmctsBot, IsmctsConfig, Player};
use libretable_conformance::rock_paper_scissors::RockPaperScissors;
use libretable_game_core::{Game, GameRng, Observer, Pending, SeatCount, Seed};
use libretable_mau_mau::{Config, MauMau};

const BUDGET: Duration = Duration::from_secs(2);

/// Random playouts from the start, counting matches and moves.
fn playouts<G: Game>(name: &str, config: &G::Config, seats: u8) {
    let seats = SeatCount::new(seats).unwrap();
    let mut driver = GameRng::from_seed(Seed::from_u64(1));
    let start = Instant::now();
    let (mut matches, mut moves) = (0_u64, 0_u64);
    while start.elapsed() < BUDGET {
        let mut rng = GameRng::from_seed(Seed::from_u64(matches));
        let mut state = G::setup(config, seats, &mut rng);
        loop {
            match G::pending(&state) {
                Pending::Finished => break,
                Pending::Chance => {
                    G::apply_chance(&mut state, &mut rng);
                }
                Pending::All(set) | Pending::Any(set) => {
                    let seat = set.iter().next().unwrap();
                    let legal = G::legal_moves(&state, seat);
                    let mv = driver.choose(&legal).unwrap().clone();
                    G::apply(&mut state, seat, &mv, &mut rng).unwrap();
                    moves += 1;
                }
            }
        }
        matches += 1;
    }
    let secs = start.elapsed().as_secs_f64();
    println!(
        "{name:<34} {:>10.0} playouts/s {:>12.0} moves/s",
        matches as f64 / secs,
        moves as f64 / secs
    );
}

/// ISMCTS iterations per second on the first decision of a match.
fn search<G: libretable_game_core::Determinize>(name: &str, config: &G::Config, seats: u8) {
    let seats = SeatCount::new(seats).unwrap();
    let mut rng = GameRng::from_seed(Seed::from_u64(1));
    let state = G::setup(config, seats, &mut rng);
    let Pending::All(set) = G::pending(&state) else {
        panic!("a seat decides first");
    };
    let seat = set.iter().next().unwrap();
    let view = G::view(&state, Observer::Seat(seat));
    let paths = G::legal_moves(&state, seat)
        .iter()
        .map(|mv| G::choices(&view, mv))
        .collect();
    let decision = Decision { seat, view, paths };
    let iterations = 1000;
    let mut bot = IsmctsBot::with_config(
        Seed::from_u64(2),
        IsmctsConfig {
            iterations,
            ..IsmctsConfig::default()
        },
    );
    let start = Instant::now();
    let mut decisions = 0_u64;
    while start.elapsed() < BUDGET {
        let _ = Player::<G>::decide(&mut bot, &decision);
        decisions += 1;
    }
    let secs = start.elapsed().as_secs_f64();
    println!(
        "{name:<34} {:>10.0} iterations/s",
        (decisions * u64::from(iterations)) as f64 / secs
    );
}

fn main() {
    playouts::<MauMau>("mau-mau, 2 seats, random playouts", &Config::default(), 2);
    playouts::<MauMau>("mau-mau, 4 seats, random playouts", &Config::default(), 4);
    playouts::<RockPaperScissors>("rps, 2 seats, random playouts", &Default::default(), 2);
    search::<MauMau>("mau-mau, 2 seats, ISMCTS", &Config::default(), 2);
    search::<MauMau>("mau-mau, 4 seats, ISMCTS", &Config::default(), 4);
}
