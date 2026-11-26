//! Deterministic game core: the `Game` trait, views, table model, selection
//! protocol, `GameRng` and game records (see `docs/architecture.md`).
//!
//! The crate is `no_std` so it builds unchanged for native, Wasm and mobile
//! targets and cannot reach I/O, clocks, threads or global randomness.
//!
//! A game is fully defined by its config, its seat count, a [`Seed`] and the
//! list of moves; [`replay`] turns a [`GameRecord`] back into a state.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod game;
mod outcome;
mod pending;
mod record;
mod rng;
mod seat;

pub use game::{Game, GameId, IllegalMove, InvalidVariant, Variant};
pub use outcome::{Outcome, SeatResult};
pub use pending::Pending;
pub use record::{GameRecord, RECORD_SCHEMA_VERSION, RecordOf, RecordedMove, ReplayError, replay};
pub use rng::{GameRng, Seed};
pub use seat::{MAX_SEATS, Observer, Seat, SeatCount, SeatSet};
