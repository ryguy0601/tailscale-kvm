#!/usr/bin/env bash
set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
cd "$DIR"

echo "=== Building Tailscale Universal KVM ==="
cargo build --release --workspace

echo ""
echo "Build complete! Artifacts available in target/release/:"
echo " - target/release/tailscale-kvm-host (Universal Host Controller)"
echo " - target/release/tailscale-kvm-receiver (Universal Receiver Daemon)"
