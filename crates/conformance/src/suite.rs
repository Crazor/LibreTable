//! The generic conformance harness.
//!
//! [`run`] plays random matches of a game and checks, at every decision point
//! and at the end of every match:
//!
//! - **Legality:** `pending` and `outcome` agree; a pending set is never empty
//!   and holds only seats of the match; exactly the pending seats have legal
//!   moves; every legal move applies; every other candidate move, every move
//!   of a seat that is not pending and every move of a seat outside the match
//!   is rejected and leaves state and generator unchanged.
//! - **Termination:** every match finishes within the declared step bound.
//! - **Round trips:** config, state, every seat's and the spectator's view,
//!   moves, events, the outcome and the record survive JSON losslessly, and
//!   serializing twice gives identical text.
//! - **Determinism:** replaying the match's record, also after a JSON round
//!   trip, reaches the identical state and generator.
//! - **No leaks** (scaffold): a seat's view is unchanged across the states the
//!   game declares to differ from the real one only in information hidden from
//!   that seat.
//! - **Table:** every seat's and the spectator's table model has unique zone
//!   and component IDs and shows no face where its observer may
//!   not look ([`check_table`]).
//! - **Selection protocol:** for every seat, every legal move is reachable
//!   through choices and no choice path leads to an illegal move
//!   ([`check_selection`]).
//! - **Messages:** every Fluent message ID the game emits (its game ID, zone
//!   and indicator labels, message indicator values, option choices and the
//!   reasons of rejected moves) is declared in [`Suite::message_ids`], which
//!   the localization check holds against the `en` catalog.
//!
//! The binary wire encoding, opaque-ID reissue on hidden shuffles,
//! game-declared invariants and cross-target determinism (native vs. Wasm) are
//! not checked yet.

use std::fmt;

use libretable_game_core::{
    Choice, Game, GameRecord, GameRng, IndicatorValue, Observer, Pending, RecordOf, Seat,
    SeatCount, Seed, Variant, check_selection, check_table, replay,
};
use serde::{Serialize, de::DeserializeOwned};

/// Build ID written into the records the harness creates.
const BUILD: &str = "conformance";

/// How to test one game: the setups to play and the game-specific knowledge
/// the generic checks need.
///
/// A plain struct of functions rather than a trait, so a game crate can
/// describe itself without the orphan rule getting in the way.
pub struct Suite<G: Game> {
    /// Variant and seat-count combinations; every one is played
    /// [`matches`](Self::matches) times. Each must pass [`Variant::validate`].
    pub setups: Vec<(G::Config, SeatCount)>,
    /// Matches played per setup.
    pub matches: usize,
    /// Seed of the harness. Match seeds and the random move choices derive
    /// from it, so a run is reproducible.
    pub seed: Seed,
    /// Upper bound on the steps of one match: moves plus chance resolutions.
    pub step_bound: fn(&G::Config, SeatCount) -> usize,
    /// Moves to try at a decision point in addition to the legal ones, so that
    /// illegal moves are seen to be rejected. Ideally every well-formed move
    /// that could be legal for some seat at some point of the game.
    pub candidate_moves: fn(&G::State) -> Vec<G::Move>,
    /// Leak test: states that differ from the given one only in information
    /// hidden from the given seat. The view of that seat must be identical in
    /// all of them. A few random samples (drawn from the given generator) are
    /// enough; every decision point of every match is checked. `None` skips
    /// the check, for games without hidden information.
    pub hidden_variants: Option<HiddenVariants<G>>,
    /// Every Fluent message ID the game's code can emit, including its game
    /// ID (the ID of its display name). The suite fails on an emitted ID
    /// missing here; the localization check (`libretable-l10n`) fails on an
    /// ID here without an `en` message. IDs the random matches cannot reach,
    /// such as [`InvalidVariant`](libretable_game_core::InvalidVariant)
    /// reasons, must be listed too.
    pub message_ids: &'static [&'static str],
}

/// See [`Suite::hidden_variants`].
pub type HiddenVariants<G> = fn(&<G as Game>::State, Seat, &mut GameRng) -> Vec<<G as Game>::State>;

/// Which property a [`Failure`] violates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Check {
    /// A setup was rejected by [`Variant::validate`].
    Setup,
    /// Pending seats, legal moves and `apply` disagree.
    Legality,
    /// A match exceeded its step bound.
    Termination,
    /// A value did not survive serialization.
    RoundTrip,
    /// Replaying a record did not reach the same state.
    Determinism,
    /// A view changed with information hidden from its observer.
    Leak,
    /// A table model is malformed or shows what its observer may not see.
    Table,
    /// Choice paths do not select exactly the legal moves.
    Selection,
    /// The game emits a message ID missing from [`Suite::message_ids`], or
    /// that list is malformed.
    Messages,
}

/// A violated property, with the record that reproduces it: the record holds
/// the moves applied up to and including the decision point that failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// The violated property.
    pub check: Check,
    /// What went wrong.
    pub message: String,
    /// The partial record as JSON.
    pub record: String,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} check failed: {}\nrecord: {}",
            self.check, self.message, self.record
        )
    }
}

impl std::error::Error for Failure {}

/// Statistics of a passed run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Matches played.
    pub matches: usize,
    /// Moves applied, over all matches.
    pub moves: usize,
    /// Chance steps resolved, over all matches.
    pub chance_steps: usize,
    /// Decision points checked, over all matches.
    pub decision_points: usize,
    /// Hidden variants compared by the leak test.
    pub leak_variants: usize,
}

/// Plays every setup of `suite` [`matches`](Suite::matches) times with random
/// legal moves and checks every property listed in the module documentation.
///
/// # Errors
///
/// The first [`Failure`] found.
pub fn run<G: Game>(suite: &Suite<G>) -> Result<Report, Failure> {
    check_message_list::<G>(suite.message_ids)?;
    let mut seeds = GameRng::from_seed(suite.seed);
    let mut report = Report::default();
    for (config, seats) in &suite.setups {
        for _ in 0..suite.matches {
            let seed = Seed::from_u64(seeds.next_u64());
            let driver = GameRng::from_seed(Seed::from_u64(seeds.next_u64()));
            let mut run = Match::<G> {
                suite,
                record: GameRecord::new(G::ID, BUILD, config.clone(), *seats, seed),
                driver,
                report: &mut report,
            };
            run.play()?;
            report.matches += 1;
        }
    }
    Ok(report)
}

/// Whether `id` is a well-formed message ID: lowercase ASCII letters, digits
/// and single inner hyphens, like a [`GameId`](libretable_game_core::GameId).
fn is_message_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && !id.contains("--")
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The declared list is well-formed, duplicate-free and holds the game ID.
fn check_message_list<G: Game>(ids: &[&str]) -> Result<(), Failure> {
    let fail = |message: String| Failure {
        check: Check::Messages,
        message,
        record: String::new(),
    };
    for (i, id) in ids.iter().enumerate() {
        if !is_message_id(id) {
            return Err(fail(format!("malformed message ID {id:?}")));
        }
        if ids[..i].contains(id) {
            return Err(fail(format!("message ID {id:?} listed twice")));
        }
    }
    if !ids.contains(&G::ID.as_str()) {
        return Err(fail(format!(
            "game ID {:?} (its display name) is not listed",
            G::ID.as_str()
        )));
    }
    Ok(())
}

/// One match in progress.
struct Match<'a, G: Game> {
    suite: &'a Suite<G>,
    record: RecordOf<G>,
    /// Picks moves and leak samples; separate from the match's own generator.
    driver: GameRng,
    report: &'a mut Report,
}

impl<G: Game> Match<'_, G> {
    fn fail(&self, check: Check, message: impl Into<String>) -> Failure {
        Failure {
            check,
            message: message.into(),
            record: serde_json::to_string(&self.record)
                .unwrap_or_else(|error| format!("<record does not serialize: {error}>")),
        }
    }

    /// `id`, emitted as `what`, is declared in [`Suite::message_ids`].
    fn check_message(&self, what: &str, id: &str) -> Result<(), Failure> {
        if self.suite.message_ids.contains(&id) {
            Ok(())
        } else {
            Err(self.fail(
                Check::Messages,
                format!("{what} {id:?} is not in the declared message IDs"),
            ))
        }
    }

    fn play(&mut self) -> Result<(), Failure> {
        let config = self.record.variant.clone();
        let seats = self.record.seats;
        if let Err(invalid) = config.validate(seats) {
            return Err(self.fail(
                Check::Setup,
                format!(
                    "setup {config:?} with {seats:?} rejected: {}",
                    invalid.reason
                ),
            ));
        }
        self.round_trip("config", &config)?;

        let bound = (self.suite.step_bound)(&config, seats);
        let mut steps = 0;
        let mut rng = GameRng::from_seed(self.record.seed);
        let mut state = G::setup(&config, seats, &mut rng);
        loop {
            match G::pending(&state) {
                Pending::Finished => break,
                Pending::Chance => {
                    self.check_decision_point(&state, &rng)?;
                    let events = G::apply_chance(&mut state, &mut rng);
                    self.round_trip("chance events", &events)?;
                    self.report.chance_steps += 1;
                }
                Pending::All(_) | Pending::Any(_) => {
                    let legal = self.check_decision_point(&state, &rng)?;
                    let Some((seat, mv)) = self.driver.choose(&legal).cloned() else {
                        return Err(self.fail(Check::Legality, "no seat has a legal move"));
                    };
                    self.record.push(seat, mv.clone());
                    let events = G::apply(&mut state, seat, &mv, &mut rng).map_err(|e| {
                        self.fail(
                            Check::Legality,
                            format!("legal move {mv:?} of {seat:?} rejected: {}", e.reason),
                        )
                    })?;
                    self.round_trip("events", &events)?;
                    self.report.moves += 1;
                }
            }
            steps += 1;
            if steps > bound {
                return Err(self.fail(
                    Check::Termination,
                    format!("not finished after the bound of {bound} steps"),
                ));
            }
        }
        self.check_end(&state, &rng)
    }

    /// Checks a state that is not finished and returns every legal
    /// `(seat, move)` pair.
    fn check_decision_point(
        &mut self,
        state: &G::State,
        rng: &GameRng,
    ) -> Result<Vec<(Seat, G::Move)>, Failure> {
        self.report.decision_points += 1;
        let seats = self.record.seats;
        let pending = G::pending(state);
        if let Some(outcome) = G::outcome(state) {
            return Err(self.fail(
                Check::Legality,
                format!("outcome {outcome:?} while pending is {pending:?}"),
            ));
        }
        if matches!(pending, Pending::All(_) | Pending::Any(_)) {
            let set = pending.seats();
            if set.is_empty() {
                return Err(self.fail(Check::Legality, format!("{pending:?} with no seat")));
            }
            if set.intersection(seats.all()) != set {
                return Err(self.fail(
                    Check::Legality,
                    format!("{pending:?} names seats outside {seats:?}"),
                ));
            }
        }

        let mut candidates = (self.suite.candidate_moves)(state);
        let mut legal = Vec::new();
        for seat in seats.seats() {
            let moves = G::legal_moves(state, seat);
            if pending.allows(seat) && moves.is_empty() {
                return Err(self.fail(
                    Check::Legality,
                    format!("pending {seat:?} has no legal move"),
                ));
            }
            if !pending.allows(seat) && !moves.is_empty() {
                return Err(self.fail(
                    Check::Legality,
                    format!("{seat:?} is not pending but has legal moves {moves:?}"),
                ));
            }
            for (i, mv) in moves.iter().enumerate() {
                if moves[..i].contains(mv) {
                    return Err(self.fail(Check::Legality, format!("{seat:?} lists {mv:?} twice")));
                }
                self.round_trip("move", mv)?;
                self.expect_accepted(state, rng, seat, mv)?;
                if !candidates.contains(mv) {
                    candidates.push(mv.clone());
                }
                legal.push((seat, mv.clone()));
            }
        }
        for seat in seats.seats() {
            let moves = G::legal_moves(state, seat);
            for mv in candidates.iter().filter(|mv| !moves.contains(mv)) {
                self.expect_rejected(state, rng, seat, mv)?;
            }
        }
        // A seat beyond the match, unless the match uses every seat.
        if let Some(outside) = Seat::new(seats.get()) {
            if !G::legal_moves(state, outside).is_empty() {
                return Err(self.fail(
                    Check::Legality,
                    format!("{outside:?} outside {seats:?} has legal moves"),
                ));
            }
            for mv in &candidates {
                self.expect_rejected(state, rng, outside, mv)?;
            }
        }

        self.check_state(state)?;
        // After legality and the table, so those failures are reported as
        // such rather than as a selection failure.
        for seat in seats.seats() {
            check_selection::<G>(state, seat, rng).map_err(|violation| {
                self.fail(
                    Check::Selection,
                    format!("selection of {seat:?}: {violation:?}"),
                )
            })?;
        }
        self.check_options(state, &legal)?;
        Ok(legal)
    }

    /// Every option choice of every legal move is a declared message ID.
    ///
    /// `legal` lists each seat's moves contiguously (see
    /// [`Self::check_decision_point`]), so each seat's view is built once.
    fn check_options(&self, state: &G::State, legal: &[(Seat, G::Move)]) -> Result<(), Failure> {
        for group in legal.chunk_by(|a, b| a.0 == b.0) {
            let view = G::view(state, Observer::Seat(group[0].0));
            for (_, mv) in group {
                for choice in G::choices(&view, mv) {
                    if let Choice::Option(id) = choice {
                        self.check_message("option choice", &id)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Round trips, table models and the leak test, for any reached state.
    fn check_state(&mut self, state: &G::State) -> Result<(), Failure> {
        self.round_trip("state", state)?;
        let spectator = G::view(state, Observer::Spectator);
        self.round_trip("spectator view", &spectator)?;
        self.check_table(&spectator, None)?;
        for seat in self.record.seats.seats() {
            let view = G::view(state, Observer::Seat(seat));
            self.round_trip("seat view", &view)?;
            self.check_table(&view, Some(seat))?;
            if let Some(hidden_variants) = self.suite.hidden_variants {
                for variant in hidden_variants(state, seat, &mut self.driver) {
                    self.report.leak_variants += 1;
                    let other = G::view(&variant, Observer::Seat(seat));
                    if other != view {
                        return Err(self.fail(
                            Check::Leak,
                            format!(
                                "view of {seat:?} changed with hidden information:\n\
                                 real {view:?}\nvariant {other:?}"
                            ),
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn check_table(&self, view: &G::View, seat: Option<Seat>) -> Result<(), Failure> {
        // `TableModel` is a game-core type whose serialization a game cannot
        // change, so it is not round-tripped here (game-core tests it).
        let table = G::table(view);
        check_table(&table, seat).map_err(|violation| {
            self.fail(Check::Table, format!("table of {seat:?}: {violation:?}"))
        })?;
        for zone in &table.zones {
            self.check_message("zone label", &zone.label)?;
        }
        for indicator in &table.indicators {
            self.check_message("indicator label", &indicator.label)?;
            if let IndicatorValue::Message(id) = &indicator.value {
                self.check_message("indicator value", id)?;
            }
        }
        Ok(())
    }

    fn expect_accepted(
        &self,
        state: &G::State,
        rng: &GameRng,
        seat: Seat,
        mv: &G::Move,
    ) -> Result<(), Failure> {
        let mut probe = state.clone();
        let mut probe_rng = rng.clone();
        G::apply(&mut probe, seat, mv, &mut probe_rng).map_err(|e| {
            self.fail(
                Check::Legality,
                format!("legal move {mv:?} of {seat:?} rejected: {}", e.reason),
            )
        })?;
        Ok(())
    }

    fn expect_rejected(
        &self,
        state: &G::State,
        rng: &GameRng,
        seat: Seat,
        mv: &G::Move,
    ) -> Result<(), Failure> {
        let mut probe = state.clone();
        let mut probe_rng = rng.clone();
        let Err(illegal) = G::apply(&mut probe, seat, mv, &mut probe_rng) else {
            return Err(self.fail(
                Check::Legality,
                format!("move {mv:?} of {seat:?} is not listed as legal but applies"),
            ));
        };
        if probe != *state || probe_rng != *rng {
            return Err(self.fail(
                Check::Legality,
                format!("rejected move {mv:?} of {seat:?} changed the state or generator"),
            ));
        }
        self.check_message("illegal-move reason", illegal.reason)
    }

    fn check_end(&mut self, state: &G::State, rng: &GameRng) -> Result<(), Failure> {
        let seats = self.record.seats;
        let Some(outcome) = G::outcome(state) else {
            return Err(self.fail(Check::Legality, "finished without an outcome"));
        };
        if outcome.seats.len() != usize::from(seats.get()) {
            return Err(self.fail(
                Check::Legality,
                format!(
                    "outcome has {} seat results for {seats:?}",
                    outcome.seats.len()
                ),
            ));
        }
        self.round_trip("outcome", &outcome)?;
        for seat in seats.seats() {
            let moves = G::legal_moves(state, seat);
            if !moves.is_empty() {
                return Err(self.fail(
                    Check::Legality,
                    format!("{seat:?} has legal moves {moves:?} after the end"),
                ));
            }
            for mv in (self.suite.candidate_moves)(state) {
                self.expect_rejected(state, rng, seat, &mv)?;
            }
        }
        self.check_state(state)?;

        let record = self.round_trip("record", &self.record)?;
        for (label, record) in [("record", &self.record), ("deserialized record", &record)] {
            match replay::<G>(record) {
                Ok((replayed, replayed_rng)) if replayed == *state && replayed_rng == *rng => {}
                Ok((replayed, _)) => {
                    return Err(self.fail(
                        Check::Determinism,
                        format!(
                            "replaying the {label} reached another state or generator:\n\
                             played {state:?}\nreplayed {replayed:?}"
                        ),
                    ));
                }
                Err(error) => {
                    return Err(self.fail(
                        Check::Determinism,
                        format!("replaying the {label} failed: {error:?}"),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Serializes `value` to JSON and back; returns the deserialized copy.
    fn round_trip<T>(&self, what: &str, value: &T) -> Result<T, Failure>
    where
        T: Serialize + DeserializeOwned + PartialEq + fmt::Debug,
    {
        let fail = |message: String| self.fail(Check::RoundTrip, format!("{what}: {message}"));
        let json = serde_json::to_string(value).map_err(|e| fail(format!("serialize: {e}")))?;
        let back: T = serde_json::from_str(&json).map_err(|e| fail(format!("deserialize: {e}")))?;
        if back != *value {
            return Err(fail(format!("{value:?} came back as {back:?}")));
        }
        let again = serde_json::to_string(&back).map_err(|e| fail(format!("serialize: {e}")))?;
        if again != json {
            return Err(fail(format!("serialized differently: {json} vs. {again}")));
        }
        Ok(back)
    }
}
