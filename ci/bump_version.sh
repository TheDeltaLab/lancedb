#!/usr/bin/env bash
set -euo pipefail

RELEASE_TYPE=${1:-preview}
BUMP_MINOR=${2:-false}
TAG_PREFIX=${3:-v}
[[ "$RELEASE_TYPE" == preview || "$RELEASE_TYPE" == stable ]] || { echo "Expected preview or stable" >&2; exit 1; }
[[ "$BUMP_MINOR" == true || "$BUMP_MINOR" == false ]] || { echo "Expected true or false" >&2; exit 1; }
[[ "$TAG_PREFIX" == v ]] || { echo "Only Node/Rust v-prefixed releases are supported" >&2; exit 1; }
[[ -z $(git status --porcelain) ]] || { echo "Release requires a clean checkout" >&2; exit 1; }

SELF_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
HEAD_SHA=$(git rev-parse HEAD)
if [[ "$RELEASE_TYPE" == stable ]]; then
  python "$SELF_DIR/validate_stable_lance.py"
fi

CURRENT_VERSION=$(bump-my-version show current_version)
if [[ "$BUMP_MINOR" == true ]]; then
  PART=minor
elif [[ "$CURRENT_VERSION" == *-beta.* ]]; then
  PART=pre_n
else
  PART=patch
fi
# Do not create intermediate commits or tags. The tag must include both locks.
bump-my-version bump --no-commit --no-tag "$PART"
if [[ "$RELEASE_TYPE" == stable ]]; then
  bump-my-version bump --no-commit --no-tag pre_l
fi
NEW_VERSION=$(bump-my-version show current_version)
NEW_TAG="v$NEW_VERSION"
if git rev-parse --verify --quiet "refs/tags/$NEW_TAG" >/dev/null; then
  echo "Release tag already exists: $NEW_TAG" >&2
  exit 1
fi
LAST_STABLE_RELEASE=$(git describe --first-parent --tags --match 'v[0-9]*' --exclude '*-*' --abbrev=0 "$HEAD_SHA")
[[ -n "$LAST_STABLE_RELEASE" ]] || { echo "No stable release baseline found" >&2; exit 1; }
python "$SELF_DIR/check_breaking_changes.py" "$LAST_STABLE_RELEASE" "$HEAD_SHA" "${LAST_STABLE_RELEASE#v}" "$NEW_VERSION"
bash "$SELF_DIR/update_lockfiles.sh"

git add .bumpversion.toml Cargo.lock nodejs/pnpm-lock.yaml nodejs/package.json \
  nodejs/Cargo.toml rust/lancedb/Cargo.toml nodejs/npm/*/package.json
git commit -m "chore(release): bump version to $NEW_VERSION"
git tag -a "$NEW_TAG" -m "Release $NEW_VERSION"
echo "Created $NEW_TAG at $(git rev-parse HEAD); nothing has been pushed."
