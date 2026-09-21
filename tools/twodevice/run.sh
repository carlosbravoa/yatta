#!/usr/bin/env bash
# Run one yatta instance as a named device.
#
#   ./run.sh laptop        # in one terminal
#   ./run.sh desktop       # in another
#
# XDG_CONFIG_HOME is what makes them two devices rather than two windows: each
# gets its own settings.json, so each points at its own vault. YATTA_DEVICE is
# the name that ends up in commit messages and conflict markers.
set -euo pipefail

DEVICE="${1:?usage: run.sh laptop|desktop}"
BENCH="${BENCH:-$HOME/yatta-twodevice}"
# The repo is wherever this script lives, two levels up -- not a hardcoded path.
REPO="${REPO:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
BIN="$REPO/src-tauri/target/release/yatta"

[ -x "$BIN" ] || { echo "build it first:  cd $REPO && npm run build && cargo build --release --features custom-protocol --manifest-path src-tauri/Cargo.toml"; exit 1; }
[ -d "$BENCH/$DEVICE" ] || { echo "no vault at $BENCH/$DEVICE -- run setup.sh"; exit 1; }

export XDG_CONFIG_HOME="$BENCH/config-$DEVICE"
export YATTA_DEVICE="$DEVICE"
exec "$BIN"
