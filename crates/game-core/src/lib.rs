//! Deterministic game core: the `Game` trait, views, table model, selection
//! protocol, `GameRng` and game records (see `docs/architecture.md`).
//!
//! The crate is `no_std` so it builds unchanged for native, Wasm and mobile
//! targets and cannot reach I/O, clocks, threads or global randomness.

#![no_std]
#![forbid(unsafe_code)]
