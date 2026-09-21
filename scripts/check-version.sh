#!/usr/bin/env bash
# Verify that every declaration of this project's own version agrees with the
# canonical one in [workspace.package]. With an argument, also require that the
# canonical version equals it (used to check a release tag against the manifest).
#
# Usage: scripts/check-version.sh [expected-version]
set -euo pipefail

cd "$(dirname "$0")/.."

canonical=$(cargo metadata --format-version 1 --no-deps \
  | jq -r '.packages[] | select(.name == "mds") | .version')

status=0

note() {
  echo "check-version: $1" >&2
  status=1
}

if [ -z "$canonical" ] || [ "$canonical" = "null" ]; then
  echo "check-version: cannot read the canonical version from cargo metadata" >&2
  exit 1
fi

# Every workspace member inherits the canonical version, so a member that
# declares its own is a second source of truth.
while read -r name version; do
  [ "$version" = "$canonical" ] || note "$name declares $version, canonical is $canonical"
done < <(cargo metadata --format-version 1 --no-deps | jq -r '.packages[] | "\(.name) \(.version)"')

# The path dependency carries a version requirement so that the crate stays
# packageable. It is a follower and must track the canonical version.
dep=$(grep -oP 'mds-core = \{ path = "crates/mds-core", version = "\K[^"]+' Cargo.toml || true)
if [ -n "$dep" ] && [ "$dep" != "$canonical" ]; then
  note "the mds-core path dependency requires $dep, canonical is $canonical"
fi

if [ "$#" -ge 1 ]; then
  expected=${1#v}
  [ "$canonical" = "$expected" ] || note "expected $expected, canonical is $canonical"
fi

if [ "$status" -eq 0 ]; then
  echo "check-version: all declarations agree on $canonical"
fi
exit "$status"
