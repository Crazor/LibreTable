# 0008: Project name and trademark

- **Status:** proposed
- **Date:** 2026-11-26
- **Owner:** Architect

## Context

The name is the protected brand that keeps store clones away from an open codebase. It should be technical rather than pictorial, work in English and German, and fit card, dice and board games.

## Decision (proposal)

Working name **LibreTable**. Concern: the "Libre-" prefix may signal affiliation with a movement often sceptical of AI-assisted development. Alternatives: Commontable, OpenTabletop. "OpenTable", "OpenBoard", "OpenDeck" and "PlayTable" are taken.

## Consequences

- The name appears only in the crate prefix, the bundle ID and one display-name constant, so renaming is a single PR.

## Open points

- Final name; trademark search (DPMA, EUIPO); domain and crate-name availability — before the first public release.
