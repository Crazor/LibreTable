//! The `table` binary; see the documentation of the `libretable_cli` crate.

use std::io::{self, IsTerminal};
use std::process::ExitCode;

use libretable_cli::{Io, run, tr};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut input = io::stdin().lock();
    let stdout = io::stdout();
    let terminal = stdout.is_terminal();
    let mut output = stdout.lock();
    let mut io = Io::new(&mut input, &mut output, terminal);
    match run(&args, &mut io) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}", tr!("cli-error", message = error.message));
            if error.usage {
                eprintln!("{}", tr!("cli-usage"));
                ExitCode::from(2)
            } else {
                ExitCode::FAILURE
            }
        }
    }
}
