#!/usr/bin/env bash
# Build the web app into clickplanet-webapp/dist: the Dioxus interface bundle (Dioxus CLI) and
# the globe bundle (clickplanet-globe, built with cargo + wasm-bindgen into dist/globe/). The two
# bundles talk through the clickplanet-globe-protocol messages.
#
# The Dioxus CLI binary is called `dx`, which clashes with Deno's `dx` on some
# machines (e.g. Homebrew). This script never trusts the first `dx` on PATH: it
# picks the first candidate that reports the expected Dioxus version.
#
# Install the right CLI with:
#   cargo install dioxus-cli --version 0.6.3 --locked
# or point DX at a specific binary: DX=/path/to/dx ./build-web.sh
#
# The globe needs wasm-bindgen-cli with the same version as the wasm-bindgen crate in Cargo.lock:
#   cargo install wasm-bindgen-cli --version 0.2.129 --locked
# wasm-opt (binaryen) is used on the globe bundle when it is installed.
#
# Usage: ./build-web.sh [--release]   (extra arguments are passed to `dx build`)
set -euo pipefail

DIOXUS_CLI_VERSION="0.6.3"
WASM_BINDGEN_VERSION="$(grep -A1 '^name = "wasm-bindgen"$' ../Cargo.lock | sed -n 's/^version = "\(.*\)"$/\1/p')"
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

if ! wasm-bindgen --version 2>/dev/null | grep -q " ${WASM_BINDGEN_VERSION}$"; then
    echo "error: wasm-bindgen-cli ${WASM_BINDGEN_VERSION} not found (found: $(wasm-bindgen --version 2>/dev/null || echo none))." >&2
    echo "install it with: cargo install wasm-bindgen-cli --version ${WASM_BINDGEN_VERSION} --locked" >&2
    exit 1
fi

"$DX_BIN" build --platform web "$@"

out="../target/dx/clickplanet-webapp/${profile}/web/public"
rm -rf dist
mkdir -p dist
cp -r "$out"/. dist/
echo "interface bundle copied to clickplanet-webapp/dist"

# Globe bundle
cargo_profile_flag=""
[ "$profile" = "release" ] && cargo_profile_flag="--release"
cargo build -p clickplanet-globe --lib --target wasm32-unknown-unknown $cargo_profile_flag
mkdir -p dist/globe
wasm-bindgen --target web --no-typescript --out-dir dist/globe \
    "../target/wasm32-unknown-unknown/${profile}/clickplanet_globe.wasm"
if [ "$profile" = "release" ] && command -v wasm-opt >/dev/null; then
    wasm-opt -O3 --all-features dist/globe/clickplanet_globe_bg.wasm -o dist/globe/clickplanet_globe_bg.wasm
fi
cp public/globe/worker.js dist/globe/worker.js
echo "globe bundle copied to clickplanet-webapp/dist/globe"
