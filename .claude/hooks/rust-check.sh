#!/usr/bin/env bash
# Stop hook: format and lint the Rust workspace before Claude finishes.
# Exit code 2 sends stderr back to Claude so it fixes the findings.
set -u

input="$(cat)"
# Avoid loops: if this stop was already triggered by this hook, let Claude stop.
if printf '%s' "$input" | grep -q '"stop_hook_active"[[:space:]]*:[[:space:]]*true'; then
  exit 0
fi

cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
[ -f Cargo.toml ] || exit 0
command -v cargo >/dev/null 2>&1 || exit 0

cargo fmt --all >/dev/null 2>&1

if ! out="$(cargo clippy --workspace --all-targets --quiet -- -D warnings 2>&1)"; then
  printf 'cargo clippy found problems; fix them before finishing:\n%s\n' "$out" >&2
  exit 2
fi

exit 0
