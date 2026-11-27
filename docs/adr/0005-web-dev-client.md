# 0005: Web dev client — dev server or Wasm

- **Status:** proposed
- **Date:** 2026-11-26
- **Owner:** Claude proposes

## Context

The web dev multiclient shows several seat views plus the omniscient view and lets Claude and the architect test games in a browser during the core stage.

## Decision (proposal)

Start with a small local dev server (Rust, WebSocket) and a minimal HTML/JS page. The dev server is the seed of the real server; Wasm is added in the web stage for offline play and on-device bots.

Refinement from implementing the dev server (milestone 1, issue 7), proposed by Claude on 2026-11-27:

- **Plain HTML, CSS and JavaScript, no framework and no build step.** The page (`apps/web/dev/`) only renders table models and prompts from the server and forwards choices, so a framework would add tooling without removing much code. The server embeds the files at compile time, so `cargo run -p libretable-server --features dev` is all it takes. The public web client of the web stage decides its own tooling.
- The protocol between page and server is [ADR-0010](0010-websocket-protocol.md); details in [design/server.md](../design/server.md).

## Consequences

- The WebSocket protocol is exercised from the first milestone.
- Omniscient views are served only by the dev server, behind the `dev` feature.
- No Node.js toolchain in the repository or CI for now.

## Open points

- None for the dev client; frontend tooling for the public web client is decided in the web stage.
