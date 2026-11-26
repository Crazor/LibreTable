#!/usr/bin/env bash
# Fails when a non-merge commit in the given range lacks a DCO
# `Signed-off-by:` trailer matching its author (see docs/adr/0002-licences.md).
#
# Usage: scripts/check-dco.sh <base>..<head>
set -euo pipefail

range="${1:?usage: $0 <base>..<head>}"
status=0
count=0

for sha in $(git rev-list --no-merges "$range"); do
  count=$((count + 1))
  author="$(git log -1 --format='%an <%ae>' "$sha")"
  if git log -1 --format='%(trailers:key=Signed-off-by,valueonly)' "$sha" \
      | grep -Fxq -- "$author"; then
    echo "ok      $(git log -1 --format='%h %s' "$sha")"
  else
    echo "MISSING $(git log -1 --format='%h %s' "$sha")"
    echo "        expected: Signed-off-by: $author"
    status=1
  fi
done

if [ "$status" -ne 0 ]; then
  echo
  echo "Some commits lack a DCO sign-off matching their author."
  echo "Sign them off with 'git commit -s' (or 'git rebase --signoff <base>') and force-push."
fi
echo "Checked $count commit(s)."
exit "$status"
