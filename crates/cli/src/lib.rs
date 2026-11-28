//! The command-line runner (binary `table`): play a match in the terminal
//! with human or bot seats, replay a JSON game record, simulate bot-only
//! matches. The [`render`] module is the text renderer.
//!
//! Matches run through `libretable-session`; human input goes through the
//! selection protocol, so no illegal move can be entered. All user-facing
//! text goes through [`l10n::tr`].

#![forbid(unsafe_code)]

pub mod commands;
pub mod l10n;
pub mod registry;
pub mod render;

use std::io::{BufRead, Write};
use std::path::PathBuf;

use commands::{PlayOptions, ReplayOptions, SeatKind, SimulateOptions};

/// Build ID written into records: the `BUILD_ID` environment
/// variable at compile time (CI sets the git commit), else the crate version.
pub const BUILD: &str = match option_env!("BUILD_ID") {
    Some(build) => build,
    None => concat!("cli-", env!("CARGO_PKG_VERSION")),
};

/// A failed command, with its message already translated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliError {
    /// What went wrong.
    pub message: String,
    /// Whether the command line was wrong, so usage should be shown.
    pub usage: bool,
}

impl CliError {
    /// A runtime error.
    #[must_use]
    pub fn new(message: String) -> Self {
        Self {
            message,
            usage: false,
        }
    }

    /// An error in the command line.
    #[must_use]
    pub fn usage(message: String) -> Self {
        Self {
            message,
            usage: true,
        }
    }
}

/// The terminal: input lines and output text.
pub struct Io<'a> {
    input: &'a mut dyn BufRead,
    output: &'a mut dyn Write,
    terminal: bool,
}

impl<'a> Io<'a> {
    /// Wraps input and output. `terminal` says whether the output is an
    /// interactive terminal, where the privacy screen clears it.
    pub fn new(input: &'a mut dyn BufRead, output: &'a mut dyn Write, terminal: bool) -> Self {
        Self {
            input,
            output,
            terminal,
        }
    }

    /// Writes `text` as is and flushes.
    ///
    /// # Errors
    ///
    /// [`CliError`] on I/O failure.
    pub fn write(&mut self, text: &str) -> Result<(), CliError> {
        self.output
            .write_all(text.as_bytes())
            .and_then(|()| self.output.flush())
            .map_err(io_error)
    }

    /// Writes `text` and a newline.
    ///
    /// # Errors
    ///
    /// [`CliError`] on I/O failure.
    pub fn line(&mut self, text: &str) -> Result<(), CliError> {
        self.write(&format!("{text}\n"))
    }

    /// Reads one line without its line ending and surrounding blanks; `None`
    /// at end of input.
    ///
    /// # Errors
    ///
    /// [`CliError`] on I/O failure.
    pub fn read_line(&mut self) -> Result<Option<String>, CliError> {
        let mut line = String::new();
        match self.input.read_line(&mut line).map_err(io_error)? {
            0 => Ok(None),
            _ => Ok(Some(line.trim().to_owned())),
        }
    }

    /// Clears an interactive terminal, so the next player cannot scroll back
    /// to the previous player's hand. Does nothing otherwise.
    ///
    /// # Errors
    ///
    /// [`CliError`] on I/O failure.
    pub fn clear_screen(&mut self) -> Result<(), CliError> {
        if self.terminal {
            // Clear screen and scrollback, cursor home.
            self.write("\x1b[2J\x1b[3J\x1b[H")
        } else {
            Ok(())
        }
    }
}

fn io_error(error: std::io::Error) -> CliError {
    CliError::new(tr!("cli-error-io", reason = error.to_string()))
}

/// Runs the command line `args` (without the program name). `--lang <tag>`
/// may appear anywhere and picks the locale (see [`l10n::init`]).
///
/// # Errors
///
/// [`CliError`] if the command fails.
pub fn run(args: &[String], io: &mut Io<'_>) -> Result<(), CliError> {
    let mut args = args.to_vec();
    let lang = match args.iter().position(|arg| arg == "--lang") {
        Some(index) if index + 1 < args.len() => {
            let tag = args.remove(index + 1);
            args.remove(index);
            Some(tag)
        }
        Some(_) => {
            l10n::init(None);
            return Err(CliError::usage(tr!(
                "cli-error-option-value",
                option = "--lang"
            )));
        }
        None => None,
    };
    l10n::init(lang.as_deref());

    let Some((command, rest)) = args.split_first() else {
        return io.line(&tr!("cli-usage"));
    };
    match command.as_str() {
        "help" | "--help" | "-h" => io.line(&tr!("cli-usage")),
        "games" => {
            Parsed::parse(rest, &[], None)?;
            io.line(&tr!("cli-games-header"))?;
            for game in registry::games() {
                let entry = tr!("cli-games-entry", id = game.id(), name = tr!(game.id()));
                io.line(&format!("  {entry}"))?;
            }
            Ok(())
        }
        "play" => {
            let parsed = Parsed::parse(
                rest,
                &["--seats", "--seed", "--variant", "--record"],
                Some("<game>"),
            )?;
            let game = game(&parsed.positional[0])?;
            let seats = match parsed.value("--seats") {
                Some(list) => list.split(',').map(seat_kind).collect::<Result<_, _>>()?,
                None => vec![SeatKind::Human, SeatKind::Bot],
            };
            let options = PlayOptions {
                seats,
                seed: match parsed.value("--seed") {
                    Some(_) => parsed.number("--seed")?,
                    None => clock_seed(),
                },
                variant: parsed.value("--variant").map(str::to_owned),
                record: parsed.value("--record").map(PathBuf::from),
            };
            game.play(&options, io)
        }
        "replay" => {
            let parsed = Parsed::parse(rest, &["--seat", "--each"], Some("<record.json>"))?;
            let path = PathBuf::from(&parsed.positional[0]);
            let json = std::fs::read_to_string(&path).map_err(|error| {
                CliError::new(tr!(
                    "cli-error-read",
                    path = path.display().to_string(),
                    reason = error.to_string()
                ))
            })?;
            let options = ReplayOptions {
                seat: match parsed.value("--seat") {
                    Some(_) => Some(parsed.number("--seat")?),
                    None => None,
                },
                each: parsed.flag("--each"),
                path,
            };
            let id = serde_json::from_str::<serde_json::Value>(&json)
                .ok()
                .and_then(|value| value.get("game")?.as_str().map(str::to_owned))
                .ok_or_else(|| {
                    CliError::new(tr!(
                        "cli-error-record-json",
                        path = options.path.display().to_string(),
                        reason = "game"
                    ))
                })?;
            game(&id)?.replay(&json, &options, io)
        }
        "simulate" => {
            let parsed = Parsed::parse(
                rest,
                &["-n", "--seats", "--seed", "--variant", "--max-moves"],
                Some("<game>"),
            )?;
            let game = game(&parsed.positional[0])?;
            let options = SimulateOptions {
                matches: parsed.number_or("-n", 100)?,
                seats: parsed.number_or("--seats", 2)?,
                seed: parsed.number_or("--seed", 0)?,
                variant: parsed.value("--variant").map(str::to_owned),
                max_moves: parsed.number_or("--max-moves", 100_000)?,
            };
            game.simulate(&options, io)
        }
        other => Err(CliError::usage(tr!(
            "cli-error-unknown-command",
            command = other
        ))),
    }
}

fn game(id: &str) -> Result<Box<dyn registry::GameEntry>, CliError> {
    registry::find(id).ok_or_else(|| CliError::usage(tr!("cli-error-unknown-game", game = id)))
}

fn seat_kind(text: &str) -> Result<SeatKind, CliError> {
    match text.trim() {
        "human" => Ok(SeatKind::Human),
        "bot" => Ok(SeatKind::Bot),
        other => Err(CliError::usage(tr!("cli-error-seat-kind", kind = other))),
    }
}

/// A seed for `play` without `--seed`; printed, so the match can be repeated.
fn clock_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            // The low 64 bits of the nanoseconds are plenty for a seed.
            #[allow(clippy::cast_possible_truncation)]
            let nanos = elapsed.as_nanos() as u64;
            nanos
        })
}

/// Options and positional arguments of one subcommand. Every option takes a
/// value except `--each`.
struct Parsed {
    positional: Vec<String>,
    options: Vec<(String, Option<String>)>,
}

const FLAGS: &[&str] = &["--each"];

impl Parsed {
    /// `required` names the one positional argument, if the command takes
    /// one.
    fn parse(args: &[String], allowed: &[&str], required: Option<&str>) -> Result<Self, CliError> {
        let positional = usize::from(required.is_some());
        let mut parsed = Self {
            positional: Vec::new(),
            options: Vec::new(),
        };
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            if !arg.starts_with('-') {
                parsed.positional.push(arg.clone());
                continue;
            }
            if !allowed.contains(&arg.as_str()) {
                return Err(CliError::usage(tr!(
                    "cli-error-unknown-option",
                    option = arg
                )));
            }
            let value =
                if FLAGS.contains(&arg.as_str()) {
                    None
                } else {
                    Some(args.next().cloned().ok_or_else(|| {
                        CliError::usage(tr!("cli-error-option-value", option = arg))
                    })?)
                };
            parsed.options.push((arg.clone(), value));
        }
        match parsed.positional.len() {
            n if n < positional => Err(CliError::usage(tr!(
                "cli-error-missing-argument",
                name = required.unwrap_or_default()
            ))),
            n if n > positional => Err(CliError::usage(tr!(
                "cli-error-unknown-command",
                command = &parsed.positional[positional]
            ))),
            _ => Ok(parsed),
        }
    }

    fn value(&self, name: &str) -> Option<&str> {
        self.options
            .iter()
            .rev()
            .find(|(option, _)| option == name)
            .and_then(|(_, value)| value.as_deref())
    }

    fn flag(&self, name: &str) -> bool {
        self.options.iter().any(|(option, _)| option == name)
    }

    fn number<T: std::str::FromStr>(&self, name: &str) -> Result<T, CliError> {
        let value = self.value(name).unwrap_or_default();
        value
            .parse()
            .map_err(|_| CliError::usage(tr!("cli-error-bad-number", option = name, value = value)))
    }

    fn number_or<T: std::str::FromStr>(&self, name: &str, default: T) -> Result<T, CliError> {
        match self.value(name) {
            Some(_) => self.number(name),
            None => Ok(default),
        }
    }
}
