# 0005: Web dev client — dev server or Wasm

- **Status:** proposed
- **Date:** 2026-11-26
- **Owner:** Claude proposes

## Context

The web dev multiclient shows several seat views plus the omniscient view and lets Claude and the architect test games in a browser during the core stage.

## Decision (proposal)

Start with a small local dev server (Rust, WebSocket) and a minimal HTML/JS page. The dev server is the seed of the real server; Wasm is added in the web stage for offline play and on-device bots.

## Consequences

- The WebSocket protocol is exercised from the first milestone.
- Omniscient views are served only by the dev server, behind the `dev` feature.

## Open points

- Frontend tooling for the dev page (plain JS vs. a small framework).
