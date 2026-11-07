#!/usr/bin/env bash
# Universal Host Launcher
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
exec "$DIR/target/release/tailscale-kvm-host" "$@"
