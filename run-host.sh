#!/usr/bin/env bash
# Universal Host Launcher
set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
cd "$DIR"

if [ ! -x "$DIR/target/release/tailscale-kvm-host" ]; then
    echo "[*] Host binary not found. Building release binary first..."
    "$DIR/build.sh"
fi

exec "$DIR/target/release/tailscale-kvm-host" "$@"
