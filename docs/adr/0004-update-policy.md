# 0004: Update policy — no rule versions

- **Status:** accepted
- **Date:** 2026-11-26
- **Owner:** Architect

## Context

Rule changes could break saved games, replays and running online matches. Versioning every game's rules adds lasting complexity, which is not justified during development and hardly in operation.

## Decision

- Rules are not versioned. A game record is `(game ID, variant, seed, move list)` plus the build ID (git commit) for diagnostics.
- During development, rule changes simply change behaviour; affected golden replays are re-recorded in the same PR with a note.
- Online operation uses maintenance windows; running server matches are finished or ended before an update.
- The server accepts only build-compatible clients; rule changes ship together with app releases, and the previous app build may be accepted for a short transition.
- Asynchronous matches carry the build ID; incompatible devices ask the player to update before continuing.
- Local saves from older builds are best effort: loaded if they replay to the recorded end state, otherwise marked outdated.

## Consequences

- Per-game rule versions can be introduced later if archives or correspondence play require them; the build ID keeps that option open.

## Open points

None.
