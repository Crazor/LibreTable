# 0002: Licences and contribution sign-off

- **Status:** accepted (legal review pending before the first public release)
- **Date:** 2026-11-26
- **Owner:** Architect

## Context

The project is open source, ships through the App Store, and wants hosted forks of the server to stay open. GPL-family licences are widely considered problematic for App Store distribution.

## Decision

| Part | Licence | Reason |
| --- | --- | --- |
| `game-core`, `games/*`, `bots`, `session`, `cli`, `ffi`, `wasm`, apps | MPL-2.0 | File-level copyleft, compatible with App Store distribution |
| `server` | AGPL-3.0-or-later | Hosted forks must publish their changes; "or later" allows moving to a future AGPL version without every contributor's consent |
| Artwork, decks | CC BY-SA 4.0 | Open reuse with attribution |
| Name and logo | Trademark (once registered) | Protects against store clones |
| Contributions | DCO sign-off | Each contributor certifies the right to submit the work under the project's licence |

## Consequences

- LICENSE files per licence in the repo root; each crate declares its SPDX licence in `Cargo.toml`.
- `cargo-deny` in CI enforces compatible dependency licences (allow list in `deny.toml`: MIT, MIT-0, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0, CC0-1.0, MPL-2.0); GPL code (e.g. FreeDoko, PlayAural) is used only as a reference, never copied.
- MIT/Apache-2.0 code (e.g. OpenSpiel, openschafkopf, JSkat) may be reused with attribution.

## Open points

- Lawyer review of MPL-2.0 + AGPL-3.0-or-later split and App Store terms.
- A DCO grants the project no right to relicense contributions; decide whether a CLA is needed to keep that option.
- Copyright status of AI-written code, which affects who can consent to a licence change.
