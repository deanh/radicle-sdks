#!/usr/bin/env bash
# Gate: the `node` on PATH is at or above the floor the package declares
# (package.json engines.node, >=26.5.0). The release scripts run the packaged
# consumer smoke and generate the loader with this Node, so an older one would
# test something no supported consumer runs. Exit 1 with the versions otherwise.
#
#   scripts/release/check-node-floor.sh [floor]      default 26.5.0
set -euo pipefail
floor=${1:-26.5.0}
command -v node >/dev/null || { echo "node is required (floor $floor)" >&2; exit 1; }
have=$(node -p process.versions.node) || { echo "node -p failed" >&2; exit 1; }
case "$have" in
  [0-9]*.[0-9]*.[0-9]*) ;;
  *) echo "could not parse Node version '$have'" >&2; exit 1 ;;
esac
IFS=. read -r have_major have_minor _ <<<"$have"
IFS=. read -r floor_major floor_minor _ <<<"$floor"
if [ "$have_major" -lt "$floor_major" ] || { [ "$have_major" -eq "$floor_major" ] && [ "$have_minor" -lt "$floor_minor" ]; }; then
  echo "Node $have is below the $floor floor; install Node ${floor%%.*} or newer" >&2
  exit 1
fi
