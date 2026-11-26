# 0003: Wire and storage encoding

- **Status:** proposed
- **Date:** 2026-11-26
- **Owner:** Claude proposes

## Context

Moves, views, events and game records cross the FFI boundary, the network and storage. They must be compact, deterministic and debuggable.

## Decision (proposal)

- Compact binary encoding (e.g. `postcard`) for the wire and storage.
- JSON for debugging, fixtures, golden replays and the CLI.
- A schema version in every message and record header.

## Consequences

- Round-trip tests in the conformance suite cover both encodings.

## Open points

- Whether non-Rust clients (Swift via UniFFI, web via Wasm) need the binary format or use generated types.
