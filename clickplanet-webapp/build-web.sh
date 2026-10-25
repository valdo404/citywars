#!/usr/bin/env bash
# Build the web bundle with the Dioxus CLI and copy it to clickplanet-webapp/dist.
#
# The Dioxus CLI binary is called `dx`, which clashes with Deno's `dx` on some
# machines (e.g. Homebrew). This script never trusts the first `dx` on PATH: it
# picks the first candidate that reports the expected Dioxus version.
#
# Install the right CLI with:
#   cargo install dioxus-cli --version 0.6.3 --locked
# or point DX at a specific binary: DX=/path/to/dx ./build-web.sh
#
# Usage: ./build-web.sh [--release]   (extra arguments are passed to `dx build`)
set -euo pipefail

DIOXUS_CLI_VERSION="0.6.3"
cd "$(dirname "$0")"

find_dx() {
    local candidate
    for candidate in "${DX:-}" "$(command -v dx || true)" "${CARGO_HOME:-$HOME/.cargo}/bin/dx"; do
        [ -n "$candidate" ] && [ -x "$candidate" ] || continue
        if "$candidate" --version 2>/dev/null | grep -q "^dioxus ${DIOXUS_CLI_VERSION}"; then
            echo "$candidate"
            return 0
        fi
    done
    return 1
}

if ! DX_BIN="$(find_dx)"; then
    echo "error: Dioxus CLI ${DIOXUS_CLI_VERSION} not found (a \`dx\` on PATH may be Deno's)." >&2
    echo "install it with: cargo install dioxus-cli --version ${DIOXUS_CLI_VERSION} --locked" >&2
    echo "or set DX=/path/to/dioxus/dx" >&2
    exit 1
fi
echo "using $("$DX_BIN" --version) at $DX_BIN"

# Same defaults as .cargo/config.toml, which only applies when cargo runs from this directory.
export CITYWARS_STATIC_SITE="${CITYWARS_STATIC_SITE:-https://storage.googleapis.com/lv-project-313715-clickwars-static/static}"
export RUSTFLAGS="${RUSTFLAGS:---cfg=web_sys_unstable_apis}"

profile="debug"
for arg in "$@"; do
    [ "$arg" = "--release" ] && profile="release"
done

"$DX_BIN" build --platform web "$@"

out="../target/dx/clickplanet-webapp/${profile}/web/public"
rm -rf dist
mkdir -p dist
cp -r "$out"/. dist/
echo "web bundle copied to clickplanet-webapp/dist"
