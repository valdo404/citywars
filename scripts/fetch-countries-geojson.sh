#!/usr/bin/env bash
#
# Download countries.geojson (country borders used by the robots' GeoLookup).
#
# Source: datasets/geo-countries (derived from Natural Earth admin 0, public
# domain / ODC-PDDL), pinned to the commit the file was originally taken from.
# The checksum matches the copy that used to be versioned in this repository.
#
# Usage:
#   ./scripts/fetch-countries-geojson.sh [destination]   # default: ./countries.geojson at the repo root
#
set -euo pipefail

URL="https://raw.githubusercontent.com/datasets/geo-countries/cbd21e172776299d78a395cca2243917d505a608/data/countries.geojson"
SHA256="764f259082c141debd112178117f2a76fe40c585415b1edd1466a43d20d15fea"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${1:-$REPO_ROOT/countries.geojson}"

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

if [ -f "$DEST" ] && [ "$(sha256_of "$DEST")" = "$SHA256" ]; then
  echo "$DEST is already up to date"
  exit 0
fi

TMP="$(mktemp "${DEST}.XXXXXX")"
trap 'rm -f "$TMP"' EXIT

echo "Downloading $URL"
curl -fsSL --retry 3 -o "$TMP" "$URL"

ACTUAL="$(sha256_of "$TMP")"
if [ "$ACTUAL" != "$SHA256" ]; then
  echo "Checksum mismatch: expected $SHA256, got $ACTUAL" >&2
  exit 1
fi

mv "$TMP" "$DEST"
trap - EXIT
echo "Saved $DEST"
