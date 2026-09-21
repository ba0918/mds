#!/usr/bin/env bash
# Print the CHANGELOG body for one heading, without the heading itself.
#
# Usage: scripts/changelog-section.sh <version|Unreleased>
set -euo pipefail

cd "$(dirname "$0")/.."

heading=${1#v}

awk -v want="$heading" '
  /^## / {
    # Strip the link form "## [0.1.0] - date" down to the version.
    title = $0
    sub(/^## +/, "", title)
    sub(/^\[/, "", title)
    sub(/\].*$/, "", title)
    sub(/ +- .*$/, "", title)
    inside = (title == want)
    next
  }
  # Link definitions live at the foot of the file; they are not release notes.
  inside && /^\[[^]]+\]: https?:\/\// { next }
  inside { print }
' CHANGELOG.md | sed -e '/./,$!d' | awk 'BEGIN{n=0} {lines[n++]=$0} END{while (n>0 && lines[n-1] ~ /^[[:space:]]*$/) n--; for (i=0;i<n;i++) print lines[i]}'
