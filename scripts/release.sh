#!/usr/bin/env bash
# Cut a release: set the canonical version, promote the unreleased changelog
# section under that version, run the project's gates, commit, tag and push.
#
# The tag confirms verification, so the gates run before it exists. Pushing the
# tag is what triggers the GitHub release; nothing here talks to a registry.
#
# Usage: scripts/release.sh <version>        e.g. scripts/release.sh 0.2.0
set -euo pipefail

cd "$(dirname "$0")/.."

die() { echo "release: $1" >&2; exit 1; }

[ "$#" -eq 1 ] || die "usage: scripts/release.sh <version>"
version=${1#v}
echo "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$' \
  || die "not a semantic version: $version"
tag="v$version"

repo=$(cargo metadata --format-version 1 --no-deps \
  | jq -r '.packages[] | select(.name == "mds") | .repository')
[ -n "$repo" ] && [ "$repo" != "null" ] || die "the manifest declares no repository"

# --- the state a release may be cut from -------------------------------------

[ -z "$(git status --porcelain)" ] || die "the working tree has changes; commit or stash them first"

branch=$(git branch --show-current)
[ "$branch" = "main" ] || die "releases are cut from main, not $branch"

git fetch --quiet origin main
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] \
  || die "main and origin/main differ; push or pull first"

# A published tag is fixed. Re-cutting one would make a name mean two states.
git rev-parse --verify --quiet "refs/tags/$tag" >/dev/null && die "$tag already exists locally"
git ls-remote --exit-code --tags origin "$tag" >/dev/null 2>&1 && die "$tag is already published"

previous=$(git tag --list 'v*' --sort=-v:refname | head -1)

# --- version and changelog, as one change ------------------------------------

current=$(cargo metadata --format-version 1 --no-deps \
  | jq -r '.packages[] | select(.name == "mds") | .version')
echo "release: $current -> $version"

perl -0pi -e "s/(\\[workspace\\.package\\]\\nversion = \")[^\"]+(\")/\${1}$version\${2}/" Cargo.toml
perl -0pi -e "s/(mds-core = \\{ path = \"crates\\/mds-core\", version = \")[^\"]+(\")/\${1}$version\${2}/" Cargo.toml

grep -q '^## Unreleased$' CHANGELOG.md || die "CHANGELOG.md has no '## Unreleased' section"
[ -n "$(./scripts/changelog-section.sh Unreleased)" ] \
  || die "the unreleased section is empty; there is nothing to release"

today=$(date +%Y-%m-%d)
if [ -n "$previous" ]; then
  compare="$repo/compare/$previous...$tag"
else
  compare="$repo/releases/tag/$tag"
fi

perl -0pi -e "s/^## Unreleased\\n/## Unreleased\\n\\n## [$version] - $today\\n/m" CHANGELOG.md

# The comparison link lives beside the heading it belongs to, appended once.
printf '\n[%s]: %s\n' "$version" "$compare" >> CHANGELOG.md
perl -0pi -e 's/\n{3,}\z/\n/' CHANGELOG.md

# --- verification, then the tag ----------------------------------------------

cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
./scripts/check-version.sh "$version"
cargo run --quiet --locked -- check ./

git add Cargo.toml Cargo.lock crates/mds-core/Cargo.toml CHANGELOG.md
git commit -m "chore: $version をリリースする" -m "変更点は CHANGELOG.md の $version の節にある。"
git tag -a "$tag" -m "$tag"

git push origin main
git push origin "$tag"

echo "release: pushed $tag; the Release workflow creates the GitHub release"
