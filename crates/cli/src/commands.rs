//! `play`, `replay` and `simulate`, generic over the game.

use std::path::PathBuf;

use libretable_bots::RandomBot;
use libretable_game_core::{
    Choice, Game, GameRng, InvalidVariant, Observer, RECORD_SCHEMA_VERSION, RecordOf, ReplayError,
    Seat, SeatCount, Seed, TableModel,
};
use libretable_session::{Session, SubmitError, derive_seed};

use crate::{BUILD, CliError, Io, render, tr};

/// Who sits at a seat in `play`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeatKind {
    /// Input from the terminal through the selection protocol.
    Human,
    /// A [`RandomBot`].
    Bot,
}

/// Options of `play`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayOptions {
    /// One entry per seat.
    pub seats: Vec<SeatKind>,
    /// Master seed; the match and bot seeds derive from it.
    pub seed: u64,
    /// The variant as JSON; `None` for the standard ruleset.
    pub variant: Option<String>,
    /// Where to write the record when the match ends or is abandoned.
    pub record: Option<PathBuf>,
}

/// Options of `replay`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayOptions {
    /// Path of the record, for messages.
    pub path: PathBuf,
    /// Whose table to show; `None` for the spectator.
    pub seat: Option<u8>,
    /// Show the table after every move, not only at the end.
    pub each: bool,
}

/// Options of `simulate`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulateOptions {
    /// Number of matches.
    pub matches: usize,
    /// Seats per match, all bots.
    pub seats: u8,
    /// Master seed; match and bot seeds derive from it.
    pub seed: u64,
    /// The variant as JSON; `None` for the standard ruleset.
    pub variant: Option<String>,
    /// A match that has not finished after this many moves is an error.
    pub max_moves: usize,
}

/// Plays one match in the terminal.
///
/// # Errors
///
/// [`CliError`] on invalid options or I/O failure.
pub fn play<G: Game>(options: &PlayOptions, io: &mut Io<'_>) -> Result<(), CliError> {
    let seats = seat_count(options.seats.len())?;
    let config = variant::<G>(options.variant.as_deref())?;
    let mut master = GameRng::from_seed(Seed::from_u64(options.seed));
    let match_seed = derive_seed(&mut master);
    let mut bots: Vec<Option<RandomBot>> = options
        .seats
        .iter()
        .map(|kind| {
            let bot = RandomBot::new(derive_seed(&mut master));
            (*kind == SeatKind::Bot).then_some(bot)
        })
        .collect();
    let humans = options.seats.len() - bots.iter().flatten().count();
    let mut session = Session::<G>::new(config, seats, match_seed, BUILD).map_err(invalid)?;

    io.line(&tr!(
        "cli-play-header",
        game = tr!(G::ID.as_str()),
        seats = seats.get(),
        seed = options.seed
    ))?;
    for (seat, kind) in options.seats.iter().enumerate() {
        let id = match kind {
            SeatKind::Human => "cli-seat-human",
            SeatKind::Bot => "cli-seat-bot",
        };
        io.line(&tr!(id, seat = seat))?;
    }

    // Spectator table last shown, so watchers see each change once.
    let mut shown: Option<TableModel> = None;
    let mut last_human: Option<Seat> = None;
    let mut abandoned = false;
    // The session takes the lowest pending seat first; for `All` that asks
    // every seat in turn, for `Any` it fixes the arrival order.
    while let Some(seat) = session.pending().seats().iter().next() {
        if let Some(bot) = &mut bots[usize::from(seat.index())] {
            session.play(seat, bot).map_err(rejected)?;
            if humans == 0 {
                show_changes(&session, &mut shown, io)?;
            } else {
                io.line(&tr!("cli-bot-acted", seat = seat.index()))?;
            }
            continue;
        }
        if humans > 1 && last_human != Some(seat) {
            handover(seat, io)?;
        }
        last_human = Some(seat);
        match ask(&session, seat, io)? {
            Some(path) => {
                session.submit_choices(seat, &path).map_err(rejected)?;
            }
            None => {
                abandoned = true;
                break;
            }
        }
    }

    if abandoned {
        io.line(&tr!("cli-quit"))?;
    } else {
        if shown.as_ref() != Some(&session.table(Observer::Spectator)) {
            io.write(&render::table(
                &session.table(Observer::Spectator),
                Observer::Spectator,
            ))?;
        }
        finish(&session, io)?;
    }
    if let Some(path) = &options.record {
        write_record::<G>(session.record(), path, io)?;
    }
    Ok(())
}

/// Replays a record, showing every move and the table of the chosen observer.
///
/// # Errors
///
/// [`CliError`] if the record does not parse or replay.
pub fn replay<G: Game>(
    json: &str,
    options: &ReplayOptions,
    io: &mut Io<'_>,
) -> Result<(), CliError> {
    let record: RecordOf<G> = serde_json::from_str(json).map_err(|error| {
        CliError::new(tr!(
            "cli-error-record-json",
            path = options.path.display().to_string(),
            reason = error.to_string()
        ))
    })?;
    // Check the whole record first, so a broken one fails before any output.
    Session::<G>::from_record(record.clone()).map_err(replay_error)?;

    let viewer = match options.seat {
        Some(index) => Observer::Seat(
            Seat::new(index)
                .filter(|seat| record.seats.contains(*seat))
                .ok_or_else(|| CliError::usage(tr!("cli-error-seat-range", seat = index)))?,
        ),
        None => Observer::Spectator,
    };
    io.line(&tr!(
        "cli-replay-header",
        game = tr!(G::ID.as_str()),
        seats = record.seats.get(),
        moves = record.moves.len()
    ))?;
    let mut session = Session::<G>::new(
        record.variant.clone(),
        record.seats,
        record.seed,
        record.build.clone(),
    )
    .map_err(invalid)?;
    for (number, recorded) in (1..).zip(&record.moves) {
        let view = session.view(Observer::Seat(recorded.seat));
        let path: Vec<Choice> = G::choices(&view, &recorded.mv);
        let table = G::table(&view);
        io.line(&tr!(
            "cli-replay-move",
            number = number,
            seat = recorded.seat.index(),
            choices = render::choices(&table, &path)
        ))?;
        session
            .submit_move(recorded.seat, recorded.mv.clone())
            .map_err(rejected)?;
        if options.each {
            io.write(&render::table(&session.table(viewer), viewer))?;
        }
    }
    if !options.each {
        io.write(&render::table(&session.table(viewer), viewer))?;
    }
    finish(&session, io)
}

/// Plays `matches` bot-only matches and prints wins and average score per
/// seat.
///
/// # Errors
///
/// [`CliError`] on invalid options or a match exceeding `max_moves`.
pub fn simulate<G: Game>(options: &SimulateOptions, io: &mut Io<'_>) -> Result<(), CliError> {
    let seats = seat_count(usize::from(options.seats))?;
    let config = variant::<G>(options.variant.as_deref())?;
    let mut master = GameRng::from_seed(Seed::from_u64(options.seed));
    let n = usize::from(seats.get());
    let mut wins = vec![0_usize; n];
    let mut scores = vec![0_i64; n];
    let mut moves = 0;
    for number in 1..=options.matches {
        let mut session = Session::<G>::new(config.clone(), seats, derive_seed(&mut master), BUILD)
            .map_err(invalid)?;
        let mut bots: Vec<RandomBot> = (0..n)
            .map(|_| RandomBot::new(derive_seed(&mut master)))
            .collect();
        while let Some(seat) = session.pending().seats().iter().next() {
            if session.record().moves.len() >= options.max_moves {
                return Err(CliError::new(tr!(
                    "cli-error-too-long",
                    number = number,
                    moves = options.max_moves
                )));
            }
            session
                .play(seat, &mut bots[usize::from(seat.index())])
                .map_err(rejected)?;
        }
        moves += session.record().moves.len();
        let outcome = session.outcome().expect("a finished match has an outcome");
        for (index, result) in outcome.seats.iter().enumerate().take(n) {
            wins[index] += usize::from(result.rank == 1);
            scores[index] += result.score;
        }
    }
    io.line(&tr!(
        "cli-simulate-summary",
        matches = options.matches,
        game = tr!(G::ID.as_str()),
        seats = n,
        moves = moves
    ))?;
    for (seat, (wins, score)) in wins.iter().zip(&scores).enumerate() {
        // Display only; precision loss on huge totals is irrelevant.
        #[allow(clippy::cast_precision_loss)]
        let average = *score as f64 / options.matches.max(1) as f64;
        io.line(&format!(
            "  {}",
            tr!(
                "cli-simulate-seat",
                seat = seat,
                wins = *wins,
                score = format!("{average:.2}")
            )
        ))?;
    }
    Ok(())
}

/// Asks a human for a whole choice path. `None` if they quit.
fn ask<G: Game>(
    session: &Session<G>,
    seat: Seat,
    io: &mut Io<'_>,
) -> Result<Option<Vec<Choice>>, CliError> {
    let viewer = Observer::Seat(seat);
    let table = session.table(viewer);
    let mut made: Vec<Choice> = Vec::new();
    let mut shown = false;
    loop {
        let prompt = session.prompt(seat, &made).map_err(rejected)?;
        if prompt.complete {
            return Ok(Some(made));
        }
        if !shown {
            io.write(&render::table(&table, viewer))?;
            shown = true;
        }
        io.write(&render::prompt(&table, seat, &made, &prompt.selectable))?;
        let Some(input) = io.read_line()? else {
            return Ok(None);
        };
        match input.as_str() {
            "q" => return Ok(None),
            "u" => {
                made.pop();
            }
            text => match text.parse::<usize>() {
                Ok(number) if (1..=prompt.selectable.len()).contains(&number) => {
                    made.push(prompt.selectable[number - 1].clone());
                }
                _ => io.line(&tr!("cli-input-invalid", max = prompt.selectable.len()))?,
            },
        }
    }
}

/// Privacy screen between human seats (pass-and-play).
fn handover(seat: Seat, io: &mut Io<'_>) -> Result<(), CliError> {
    io.clear_screen()?;
    io.line(&tr!("cli-handover", seat = seat.index()))?;
    io.read_line()?;
    Ok(())
}

fn show_changes<G: Game>(
    session: &Session<G>,
    shown: &mut Option<TableModel>,
    io: &mut Io<'_>,
) -> Result<(), CliError> {
    let table = session.table(Observer::Spectator);
    if shown.as_ref() != Some(&table) {
        io.write(&render::table(&table, Observer::Spectator))?;
        *shown = Some(table);
    }
    Ok(())
}

fn finish<G: Game>(session: &Session<G>, io: &mut Io<'_>) -> Result<(), CliError> {
    match session.outcome() {
        Some(outcome) => io.write(&render::outcome(&outcome)),
        None => io.line(&tr!("cli-unfinished")),
    }
}

fn write_record<G: Game>(
    record: &RecordOf<G>,
    path: &PathBuf,
    io: &mut Io<'_>,
) -> Result<(), CliError> {
    let json = serde_json::to_string_pretty(record).expect("records serialize") + "\n";
    std::fs::write(path, json).map_err(|error| {
        CliError::new(tr!(
            "cli-error-write",
            path = path.display().to_string(),
            reason = error.to_string()
        ))
    })?;
    io.line(&tr!(
        "cli-record-written",
        path = path.display().to_string()
    ))
}

fn seat_count(count: usize) -> Result<SeatCount, CliError> {
    u8::try_from(count)
        .ok()
        .and_then(SeatCount::new)
        .ok_or_else(|| CliError::usage(tr!("cli-error-seat-count", count = count)))
}

fn variant<G: Game>(json: Option<&str>) -> Result<G::Config, CliError> {
    json.map_or_else(
        || Ok(G::Config::default()),
        |json| {
            serde_json::from_str(json).map_err(|error| {
                CliError::usage(tr!("cli-error-variant-json", reason = error.to_string()))
            })
        },
    )
}

fn invalid(error: InvalidVariant) -> CliError {
    CliError::new(tr!("cli-error-invalid-variant", reason = tr!(error.reason)))
}

fn rejected(error: SubmitError) -> CliError {
    CliError::new(match error {
        SubmitError::NotPending(seat) => tr!("cli-error-not-pending", seat = seat.index()),
        SubmitError::NotSelectable { index } => {
            tr!("cli-error-not-selectable", number = index + 1)
        }
        SubmitError::Incomplete => tr!("cli-error-incomplete"),
        SubmitError::Ambiguous(_) => tr!("cli-error-ambiguous"),
        SubmitError::Illegal(error) => tr!("cli-error-illegal", reason = tr!(error.reason)),
    })
}

fn replay_error(error: ReplayError) -> CliError {
    CliError::new(match error {
        ReplayError::Schema { found } => tr!(
            "cli-error-replay-schema",
            found = found,
            expected = RECORD_SCHEMA_VERSION
        ),
        ReplayError::WrongGame => tr!("cli-error-replay-wrong-game"),
        ReplayError::InvalidVariant(error) => {
            tr!("cli-error-invalid-variant", reason = tr!(error.reason))
        }
        ReplayError::NotPending { index } => {
            tr!("cli-error-replay-not-pending", number = index + 1)
        }
        ReplayError::IllegalMove { index, error } => tr!(
            "cli-error-replay-illegal",
            number = index + 1,
            reason = tr!(error.reason)
        ),
    })
}
