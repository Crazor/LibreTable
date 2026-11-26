# 0006: Table renderer on Apple platforms

- **Status:** proposed
- **Date:** 2026-11-26
- **Owner:** Claude proposes

## Context

The table surface animates many components (dealing, tricks, dice). SwiftUI alone may or may not perform well enough; SpriteKit or Canvas are alternatives for the surface, with SwiftUI for the chrome.

## Decision (proposal)

Decide after an animation spike at the start of the Mac stage, measuring a full Rommé table and a dice roll.

## Consequences

- The table model stays renderer-agnostic either way.

## Open points

- Spike criteria (frame rate, accessibility support, code size).
