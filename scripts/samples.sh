#!/usr/bin/env bash
#
# Regenerate the animated samples in assets/ from fixed seeds.
# Requires agg (https://github.com/asciinema/agg):
#
#     cargo install --git https://github.com/asciinema/agg
#     # or grab a binary from https://github.com/asciinema/agg/releases
#
set -euo pipefail

cd "$(dirname "$0")/.."

COLS="${COLS:-80}"
ROWS="${ROWS:-30}"
FPS="${FPS:-20}"
WARMUP="${WARMUP:-3.0}"
DURATION="${DURATION:-5.0}"
SIZE_ORB="${SIZE_ORB:-7}"
SIZE_CARRION="${SIZE_CARRION:-10}"
FONT_SIZE="${FONT_SIZE:-12}"
SEED_ORB="${SEED_ORB:-7}"
SEED_CARRION="${SEED_CARRION:-11}"

# dark navy terminal: background, foreground, then 16 ANSI palette entries
THEME="060913,9fb6ff,0b1220,7f1d1d,166534,92400e,1e3a8a,6d28d9,0e7490,94a3b8,334155,ef4444,22c55e,f59e0b,3b82f6,a855f7,22d3ee,e2e8f0"

if ! command -v agg >/dev/null 2>&1; then
    echo "error: agg not found." >&2
    echo "install: cargo install --git https://github.com/asciinema/agg" >&2
    echo "or grab a binary from https://github.com/asciinema/agg/releases" >&2
    exit 1
fi

cargo build --release
mkdir -p target/samples assets
BIN=target/release/orbyn

capture() {
    local name="$1" seed="$2" size="$3"
    shift 3
    local cast="target/samples/$name.cast"
    local gif="assets/$name.gif"

    "$BIN" --seed "$seed" --cols "$COLS" --rows "$ROWS" --fps "$FPS" \
        --warmup "$WARMUP" --duration "$DURATION" --color truecolor \
        --size "$size" --cast "$cast" "$@"
    agg -q --theme "$THEME" --font-size "$FONT_SIZE" --fps-cap "$FPS" \
        --last-frame-duration 1 "$cast" "$gif"

    printf 'wrote %s (%s)\n' "$gif" "$(du -h "$gif" | cut -f1)"
}

capture orb "$SEED_ORB" "$SIZE_ORB"
capture carrion "$SEED_CARRION" "$SIZE_CARRION" --carrion
