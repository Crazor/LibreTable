//! The game server, starting as the dev server for the web
//! multiclient (docs/design/server.md).
//!
//! It hosts matches through `libretable-session`, plays bot seats with
//! `libretable-bots` and talks to clients over a WebSocket at `/ws` with the
//! JSON messages of [`protocol`]. It also serves the dev multiclient page
//! (`apps/web/dev`, embedded at compile time) at `/`.
//!
//! The `dev` cargo feature adds what only local development may see: the
//! omniscient view, the full game record with its seed, and game events. A
//! build without it refuses `"omniscient"` and never sends any of them.
//!
//! Licensed AGPL-3.0-or-later (docs/policies.md).

#![forbid(unsafe_code)]

mod connection;
mod error;
mod hosted;
pub mod protocol;
mod web;

pub use connection::{Change, Connection, Server};
pub use error::ServerError;
pub use hosted::Moved;
pub use web::{router, serve};

/// Whether this build has the `dev` feature.
pub const DEV: bool = cfg!(feature = "dev");

/// Build ID written into records and `welcome`: the `BUILD_ID`
/// environment variable at compile time (CI sets the git commit), else the
/// crate version.
pub const BUILD: &str = match option_env!("BUILD_ID") {
    Some(build) => build,
    None => concat!("server-", env!("CARGO_PKG_VERSION")),
};

/// Every message ID the server and its web page use, in
/// `locales/*/server.ftl`. Game messages come from the registry.
pub const MESSAGE_IDS: &[&str] = &[
    // Console.
    "server-usage",
    "server-listening",
    "server-dev-build",
    "server-bind-failed",
    "server-bad-argument",
    // Errors sent to clients.
    "server-error-protocol-version",
    "server-error-bad-message",
    "server-error-unknown-game",
    "server-error-bad-seats",
    "server-error-drivers",
    "server-error-variant-json",
    "server-error-invalid-variant",
    "server-error-unknown-match",
    "server-error-no-match",
    "server-error-bad-seat",
    "server-error-seat-is-bot",
    "server-error-no-ismcts",
    "server-error-not-pending",
    "server-error-not-selectable",
    "server-error-incomplete",
    "server-error-ambiguous",
    "server-error-illegal",
    "server-error-bad-position",
    "server-error-omniscient-unavailable",
    "server-error-internal",
    // Web page.
    "server-page-title",
    "server-page-connecting",
    "server-page-connected",
    "server-page-disconnected",
    "server-page-dev-build",
    "server-page-language",
    "server-page-game",
    "server-page-seats",
    "server-page-seed",
    "server-page-seed-random",
    "server-page-variant",
    "server-page-create",
    "server-page-match-id",
    "server-page-join",
    "server-page-match",
    "server-page-seat",
    "server-page-spectator",
    "server-page-omniscient",
    "server-page-driver",
    "server-page-driver-human",
    "server-page-driver-bot",
    "server-page-driver-ismcts",
    "server-page-to-act",
    "server-page-waiting",
    "server-page-chosen",
    "server-page-nothing-chosen",
    "server-page-choose",
    "server-page-complete",
    "server-page-undo",
    "server-page-submit",
    "server-page-owner",
    "server-page-empty",
    "server-page-face-down",
    "server-page-log",
    "server-page-log-start",
    "server-page-log-entry",
    "server-page-log-hidden",
    "server-page-first",
    "server-page-previous",
    "server-page-next",
    "server-page-last",
    "server-page-live",
    "server-page-position",
    "server-page-history",
    "server-page-outcome",
    "server-page-outcome-seat",
    "server-page-record",
    "server-page-download-record",
    "server-page-error",
];
