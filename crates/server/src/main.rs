//! `table-server [--listen <address>]`: runs the dev server.

use std::process::ExitCode;

use libretable_l10n::{Arg, Localizer};
use libretable_server::{BUILD, DEV, Server, serve};
use tokio::net::TcpListener;

/// Loopback only: the dev server has no authentication.
const DEFAULT_LISTEN: &str = "127.0.0.1:8080";

#[tokio::main]
async fn main() -> ExitCode {
    let l10n = Localizer::new(libretable_l10n::env_locale().as_deref().unwrap_or("en"));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let listen = match args.as_slice() {
        [] => DEFAULT_LISTEN.to_owned(),
        [flag, address] if flag == "--listen" => address.clone(),
        [flag] if flag == "--help" || flag == "-h" => {
            println!("{}", l10n.format("server-usage", &[]));
            return ExitCode::SUCCESS;
        }
        _ => {
            eprintln!(
                "{}",
                l10n.format(
                    "server-bad-argument",
                    &[("arguments", Arg::Text(&args.join(" ")))]
                )
            );
            eprintln!("{}", l10n.format("server-usage", &[]));
            return ExitCode::from(2);
        }
    };
    let listener = match TcpListener::bind(&listen).await {
        Ok(listener) => listener,
        Err(error) => {
            let reason = error.to_string();
            eprintln!(
                "{}",
                l10n.format(
                    "server-bind-failed",
                    &[
                        ("address", Arg::Text(&listen)),
                        ("reason", Arg::Text(&reason))
                    ]
                )
            );
            return ExitCode::FAILURE;
        }
    };
    let url = listener
        .local_addr()
        .map_or_else(|_| listen.clone(), |address| format!("http://{address}/"));
    println!(
        "{}",
        l10n.format("server-listening", &[("url", Arg::Text(&url))])
    );
    if DEV {
        println!("{}", l10n.format("server-dev-build", &[]));
    }
    match serve(listener, Server::new(BUILD)).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
