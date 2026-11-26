# Policies: privacy, legal & App Store

The platform never handles real money or transferable value, collects the minimum of personal data, and is designed to the App Store rules from the start. Legal points need review by a lawyer before the first public release.

- **No gambling:** no stakes, no purchasable or tradeable chips or points, no prizes. Age rating questions on simulated gambling are answered accordingly.
- **Data minimization (GDPR):** single player and local play need no account. Online accounts store a pseudonym, credentials via Sign in with Apple or passkeys, game records and settings; no tracking, no ads, no third-party analytics.
- **Account deletion** inside the app, as the App Store requires; game records of deleted accounts are anonymized.
- **Chat & moderation:** start with preset messages and emotes; free-text chat only between friends, with reporting and blocking.
- **Copyright & trademarks:** implement public-domain games, or publisher games only with a licence. Own artwork, own names, no imitation of existing products; commercial games may serve as references for requirements only. Where a commercial game has a public-domain ancestor (Ligretto → Nerts, Rummikub → Okey, Kniffel → Yacht), we implement the ancestor under our own name.
- **Dependencies:** every third-party crate and package must carry a licence compatible with our licences; `cargo-deny` enforces this in CI.
- **Trainer fairness:** analysis and trainer features are free in solo and bot games; in games with humans only by table agreement, and such games are unranked.
- **Project name:** chosen after a trademark search (DPMA, EUIPO) and a domain check before the first public release ([ADR-0008](adr/0008-project-name.md)).
