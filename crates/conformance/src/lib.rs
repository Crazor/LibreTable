//! Conformance suite run for every game (see `docs/architecture.md`, testing &
//! conformance suite), plus small test games that exercise parts of the
//! `Game` interface no shipped game needs yet.
//!
//! The crate is a development tool and is never shipped.

#![forbid(unsafe_code)]

pub mod rock_paper_scissors;
pub mod suite;

pub use suite::{Check, Failure, HiddenVariants, Report, Suite, run};
