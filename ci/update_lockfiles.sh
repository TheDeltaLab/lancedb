#!/usr/bin/env bash
set -euo pipefail
[[ $# -eq 0 ]] || { echo "Usage: $0 (updates locks without committing or tagging)" >&2; exit 1; }

# Update workspace versions while retaining other locked dependencies.
cargo update --workspace
(
  cd nodejs
  pnpm install --lockfile-only --ignore-scripts --no-frozen-lockfile
  pnpm install --lockfile-only --ignore-scripts --frozen-lockfile
)
