//! `RandomBot`: legal, deterministic, covers every move.

use libretable_bots::{Decision, Player, RandomBot};
use libretable_conformance::rock_paper_scissors::{RockPaperScissors, View};
use libretable_game_core::{Choice, Seat, SeatSet, Seed};

fn decision() -> Decision<View> {
    Decision {
        seat: Seat::new(0).unwrap(),
        view: View {
            rounds: 1,
            history: Vec::new(),
            scores: vec![0, 0],
            waiting: SeatSet::EMPTY,
            throws: vec![None, None],
        },
        paths: ["rps-rock", "rps-paper", "rps-scissors"]
            .map(|id| vec![Choice::Option(id.into())])
            .to_vec(),
    }
}

fn picks(seed: u64, count: usize) -> Vec<Vec<Choice>> {
    let decision = decision();
    let mut bot = RandomBot::new(Seed::from_u64(seed));
    (0..count)
        .map(|_| Player::<RockPaperScissors>::decide(&mut bot, &decision))
        .collect()
}

#[test]
fn picks_only_legal_paths_and_all_of_them() {
    let decision = decision();
    let picks = picks(1, 300);
    assert!(picks.iter().all(|path| decision.paths.contains(path)));
    for path in &decision.paths {
        let count = picks.iter().filter(|pick| *pick == path).count();
        assert!((60..=140).contains(&count), "{path:?} picked {count} times");
    }
}

#[test]
fn same_seed_same_picks() {
    assert_eq!(picks(5, 50), picks(5, 50));
    assert_ne!(picks(5, 50), picks(6, 50));
}

#[test]
fn no_paths_gives_an_empty_answer() {
    let mut decision = decision();
    decision.paths.clear();
    let mut bot = RandomBot::new(Seed::from_u64(0));
    assert!(Player::<RockPaperScissors>::decide(&mut bot, &decision).is_empty());
}
