//! Information Set Monte Carlo Tree Search (single-observer ISMCTS, Cowling,
//! Powley and Whitehouse 2012), generic over any game that implements
//! [`Determinize`].
//!
//! Each iteration samples a determinization of the bot's view, walks the
//! tree from the root along moves legal in that determinization (UCB1 with
//! availability counts), adds one new node, plays random moves to the end
//! and backs the result up. Nodes are keyed by `(seat, move)`, so the tree
//! is shared across determinizations. The answer is the root move visited
//! most.
//!
//! Rewards come from the outcome's ranks: rank 1 scores 1, the last rank 0,
//! linearly in between; a playout cut off by the step limit scores ½ for
//! everybody.

use alloc::vec::Vec;

use libretable_game_core::{Choice, Determinize, Game, GameRng, Pending, Seat, Seed};

use crate::{Decision, Player};

/// Search settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IsmctsConfig {
    /// Iterations (determinizations) per decision.
    pub iterations: u32,
    /// UCB1 exploration constant.
    pub exploration: f64,
    /// Moves after which a random playout is cut off.
    pub playout_limit: u32,
}

impl Default for IsmctsConfig {
    fn default() -> Self {
        Self {
            iterations: 500,
            exploration: 0.7,
            playout_limit: 20_000,
        }
    }
}

/// How the search rates one legal move at the root.
#[derive(Clone, Debug, PartialEq)]
pub struct MoveStats<M> {
    /// The move.
    pub mv: M,
    /// Iterations that went through it.
    pub visits: u32,
    /// Mean reward of those iterations, in `0..=1`; an estimate of the
    /// move's value for the deciding seat.
    pub value: f64,
}

/// The ISMCTS player. Deterministic for a given seed and sequence of
/// decisions.
#[derive(Clone, Debug, PartialEq)]
pub struct IsmctsBot {
    rng: GameRng,
    config: IsmctsConfig,
}

impl IsmctsBot {
    /// A bot with the default settings, drawing from its own generator
    /// seeded with `seed`.
    #[must_use]
    pub fn new(seed: Seed) -> Self {
        Self::with_config(seed, IsmctsConfig::default())
    }

    /// A bot with the given settings.
    #[must_use]
    pub fn with_config(seed: Seed, config: IsmctsConfig) -> Self {
        Self {
            rng: GameRng::from_seed(seed),
            config,
        }
    }

    /// Searches `decision` and rates every root move tried, best first.
    pub fn search<G: Determinize>(
        &mut self,
        decision: &Decision<G::View>,
    ) -> Vec<MoveStats<G::Move>> {
        let mut tree: Vec<Node<G::Move>> = alloc::vec![Node::root()];
        for _ in 0..self.config.iterations {
            self.iterate::<G>(&mut tree, decision);
        }
        let mut stats: Vec<MoveStats<G::Move>> = tree[0]
            .children
            .iter()
            .map(|&child| {
                let node = &tree[child];
                MoveStats {
                    mv: node.mv.clone().expect("children carry a move"),
                    visits: node.visits,
                    value: node.mean(),
                }
            })
            .collect();
        stats.sort_by_key(|stat| core::cmp::Reverse(stat.visits));
        stats
    }

    fn iterate<G: Determinize>(
        &mut self,
        tree: &mut Vec<Node<G::Move>>,
        decision: &Decision<G::View>,
    ) {
        let rng = &mut self.rng;
        let mut state = G::determinize(&decision.view, decision.seat, rng);
        let mut node = 0;
        let mut path = alloc::vec![0];
        // Selection and expansion.
        while let Some(mover) = next_mover::<G>(&mut state, rng) {
            let legal = G::legal_moves(&state, mover);
            let mut untried = Vec::new();
            for mv in &legal {
                let found = tree[node].children.iter().copied().find(|&child| {
                    tree[child].seat == Some(mover) && tree[child].mv.as_ref() == Some(mv)
                });
                match found {
                    Some(child) => tree[child].available += 1,
                    None => untried.push(mv.clone()),
                }
            }
            if let Some(mv) = rng.choose(&untried).cloned() {
                let child = tree.len();
                tree.push(Node {
                    mv: Some(mv.clone()),
                    seat: Some(mover),
                    children: Vec::new(),
                    visits: 0,
                    available: 1,
                    reward: 0.0,
                });
                tree[node].children.push(child);
                if G::apply(&mut state, mover, &mv, rng).is_err() {
                    return;
                }
                path.push(child);
                break;
            }
            let exploration = self.config.exploration;
            let best = tree[node]
                .children
                .iter()
                .copied()
                .filter(|&child| {
                    tree[child].seat == Some(mover)
                        && tree[child].mv.as_ref().is_some_and(|mv| legal.contains(mv))
                })
                .max_by(|&a, &b| {
                    tree[a]
                        .ucb(exploration)
                        .partial_cmp(&tree[b].ucb(exploration))
                        .unwrap_or(core::cmp::Ordering::Equal)
                });
            let Some(child) = best else { break };
            let mv = tree[child].mv.clone().expect("children carry a move");
            if G::apply(&mut state, mover, &mv, rng).is_err() {
                return;
            }
            node = child;
            path.push(child);
        }
        // Playout.
        let rewards = playout::<G>(&mut state, rng, self.config.playout_limit);
        // Backpropagation.
        for &index in &path {
            let entry = &mut tree[index];
            entry.visits += 1;
            if let Some(seat) = entry.seat {
                entry.reward += rewards
                    .get(usize::from(seat.index()))
                    .copied()
                    .unwrap_or(0.5);
            }
        }
    }
}

impl<G: Determinize> Player<G> for IsmctsBot {
    fn decide(&mut self, decision: &Decision<G::View>) -> Vec<Choice> {
        if decision.paths.len() <= 1 {
            return decision.paths.first().cloned().unwrap_or_default();
        }
        let stats = self.search::<G>(decision);
        stats
            .iter()
            .map(|stat| G::choices(&decision.view, &stat.mv))
            .find(|path| decision.paths.contains(path))
            .unwrap_or_else(|| decision.paths[0].clone())
    }
}

struct Node<M> {
    mv: Option<M>,
    /// Who made `mv`.
    seat: Option<Seat>,
    children: Vec<usize>,
    visits: u32,
    /// Iterations in which `mv` was legal at the parent.
    available: u32,
    /// Sum of the mover's rewards.
    reward: f64,
}

impl<M> Node<M> {
    const fn root() -> Self {
        Self {
            mv: None,
            seat: None,
            children: Vec::new(),
            visits: 0,
            available: 0,
            reward: 0.0,
        }
    }

    fn mean(&self) -> f64 {
        if self.visits == 0 {
            0.0
        } else {
            self.reward / f64::from(self.visits)
        }
    }

    fn ucb(&self, exploration: f64) -> f64 {
        if self.visits == 0 {
            return f64::INFINITY;
        }
        let visits = f64::from(self.visits);
        self.mean() + exploration * sqrt(ln(f64::from(self.available.max(1))) / visits)
    }
}

/// Resolves chance steps; returns the seat to move, `None` at the end.
fn next_mover<G: Game>(state: &mut G::State, rng: &mut GameRng) -> Option<Seat> {
    loop {
        match G::pending(state) {
            Pending::Finished => return None,
            Pending::Chance => {
                G::apply_chance(state, rng);
            }
            Pending::All(seats) | Pending::Any(seats) => return seats.iter().next(),
        }
    }
}

/// Plays random moves to the end; returns each seat's reward.
fn playout<G: Game>(state: &mut G::State, rng: &mut GameRng, limit: u32) -> Vec<f64> {
    for _ in 0..limit {
        let Some(mover) = next_mover::<G>(state, rng) else {
            break;
        };
        let legal = G::legal_moves(state, mover);
        let Some(mv) = rng.choose(&legal).cloned() else {
            break;
        };
        if G::apply(state, mover, &mv, rng).is_err() {
            break;
        }
    }
    let Some(outcome) = G::outcome(state) else {
        return Vec::new();
    };
    let worst = outcome.seats.iter().map(|s| s.rank).max().unwrap_or(1);
    outcome
        .seats
        .iter()
        .map(|result| {
            if worst <= 1 {
                0.5
            } else {
                f64::from(worst - result.rank) / f64::from(worst - 1)
            }
        })
        .collect()
}

/// Natural logarithm for `x > 0` without `std`: `x = m · 2^e` with
/// `m` in `[1, 2)`, `ln m = 2 atanh((m − 1) / (m + 1))`.
fn ln(x: f64) -> f64 {
    if x <= 0.0 {
        return f64::NEG_INFINITY;
    }
    let bits = x.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i64 - 1023;
    let mantissa = f64::from_bits((bits & 0x000f_ffff_ffff_ffff) | 0x3ff0_0000_0000_0000);
    let y = (mantissa - 1.0) / (mantissa + 1.0);
    let y2 = y * y;
    let mut term = y;
    let mut sum = 0.0;
    for k in 0..12 {
        sum += term / f64::from(2 * k + 1);
        term *= y2;
    }
    #[allow(clippy::cast_precision_loss)] // |exponent| < 1100.
    let exponent = exponent as f64;
    exponent * core::f64::consts::LN_2 + 2.0 * sum
}

/// Square root for `x >= 0` without `std`, by Newton's method.
fn sqrt(x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut guess = if x > 1.0 { x / 2.0 } else { 1.0 };
    for _ in 0..60 {
        let next = 0.5 * (guess + x / guess);
        if next == guess {
            break;
        }
        guess = next;
    }
    guess
}

#[cfg(test)]
mod tests {
    use super::{ln, sqrt};

    #[test]
    fn ln_and_sqrt_are_close_enough() {
        for x in [0.5, 1.0, 2.0, 10.0, 1234.5, 1e6] {
            assert!((ln(x) - std::primitive::f64::ln(x)).abs() < 1e-9, "ln {x}");
            assert!(
                (sqrt(x) - std::primitive::f64::sqrt(x)).abs() < 1e-9,
                "sqrt {x}"
            );
        }
    }
}
