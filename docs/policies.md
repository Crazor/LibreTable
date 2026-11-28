# Policies: privacy, legal & App Store

The platform never handles real money or transferable value, collects the minimum of personal data, and is designed to the App Store rules from the start. Legal points need review by a lawyer before the first public release.

- **No gambling:** no stakes, no purchasable or tradeable chips or points, no prizes. Age rating questions on simulated gambling are answered accordingly.
- **Data minimization (GDPR):** single player and local play need no account. Online accounts store a pseudonym, credentials via Sign in with Apple or passkeys, game records and settings; no tracking, no ads, no third-party analytics.
- **Account deletion** inside the app, as the App Store requires; game records of deleted accounts are anonymized.
- **Chat & moderation:** start with preset messages and emotes; free-text chat only between friends, with reporting and blocking.
- **Copyright & trademarks:** implement public-domain games, or publisher games only with a licence. Own artwork, own names, no imitation of existing products; commercial games may serve as references for requirements only. Where a commercial game has a public-domain ancestor (Ligretto → Nerts, Rummikub → Okey, Kniffel → Yacht), we implement the ancestor under our own name.
- **Dependencies:** every third-party crate and package must carry a licence compatible with our licences; `cargo-deny` enforces this in CI.
- **Trainer fairness:** analysis and trainer features are free in solo and bot games; in games with humans only by table agreement, and such games are unranked.

## Licences

The project is open source, ships through the App Store, and wants hosted forks of the server to stay open. GPL-family licences are widely considered problematic for App Store distribution. Legal review is pending before the first public release.

| Part | Licence | Reason |
| --- | --- | --- |
| `game-core`, `games/*`, `bots`, `session`, `cli`, `ffi`, `wasm`, apps | MPL-2.0 | File-level copyleft, compatible with App Store distribution |
| `server` | AGPL-3.0-or-later | Hosted forks must publish their changes; "or later" allows moving to a future AGPL version without every contributor's consent |
| Artwork, decks | CC BY-SA 4.0 | Open reuse with attribution |
| Name and logo | Trademark (once registered) | Protects against store clones |
| Contributions | DCO sign-off | Each contributor certifies the right to submit the work under the project's licence |

- LICENSE files per licence in the repo root; each crate declares its SPDX licence in `Cargo.toml`.
- `cargo-deny` in CI enforces compatible dependency licences (allow list in `deny.toml`). GPL code (e.g. FreeDoko, PlayAural) is used only as a reference, never copied; MIT/Apache-2.0 code (e.g. OpenSpiel, openschafkopf, JSkat) may be reused with attribution.
- Open: lawyer review of the MPL-2.0 + AGPL-3.0-or-later split and the App Store terms; whether a CLA is needed to keep relicensing possible (a DCO grants no such right); the copyright status of AI-written code, which affects who can consent to a licence change.

## Project name

The name is the protected brand that keeps store clones away from an open codebase. It should be technical rather than pictorial, work in English and German, and fit card, dice and board games. Working name: **LibreTable**; the "Libre-" prefix may signal affiliation with a movement often sceptical of AI-assisted development. Alternatives: Commontable, OpenTabletop ("OpenTable", "OpenBoard", "OpenDeck" and "PlayTable" are taken). The name appears only in the crate prefix, the bundle ID and one display-name constant, so renaming is a single PR. The final name is chosen after a trademark search (DPMA, EUIPO) and a domain and crate-name check before the first public release.
