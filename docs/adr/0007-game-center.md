# 0007: Game Center tier

- **Status:** proposed
- **Date:** 2026-11-26
- **Owner:** Architect

## Context

Game Center allows online play between friends without running a server, but every client (turn-based) or the host (real-time) sees the full state, so it is not safe for hidden information against manipulated apps.

## Decision (proposal)

Build Game Center as a thin transport for friends in the Mac stage; competitive and public play use the own server.

## Consequences

- Bots in Game Center matches run on the device whose turn comes next (turn-based) or on the host (real-time).
- Simultaneous decisions are serialized and sealed in the match data.

## Open points

- Whether the web-stage server makes Game Center unnecessary in practice.
