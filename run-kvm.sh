#!/usr/bin/env bash
# Single unified launcher for Tailscale Universal KVM.
# Runs Receiver daemon in background and Host controller in foreground,
# so any machine running ./run-kvm.sh can act as a Host, Receiver, or both.

set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
cd "$DIR"

MODE="${1:-all}"
if [ "$MODE" = "host" ] || [ "$MODE" = "receiver" ] || [ "$MODE" = "all" ] || [ "$MODE" = "both" ]; then
    shift || true
else
    MODE="all"
fi

HOST_BIN="$DIR/target/release/tailscale-kvm-host"
RECV_BIN="$DIR/target/release/tailscale-kvm-receiver"

if [ ! -x "$HOST_BIN" ] || [ ! -x "$RECV_BIN" ]; then
    echo "[*] Release binaries not found. Building release binaries first..."
    "$DIR/build.sh"
fi

# Clean up any existing instances
pkill -f "tailscale-kvm-host" 2>/dev/null || true
pkill -f "tailscale-kvm-receiver" 2>/dev/null || true
sleep 0.25

if [ "$MODE" = "receiver" ]; then
    echo "=== Launching Tailscale KVM Receiver Daemon ==="
    exec "$RECV_BIN" "$@"
fi

if [ "$MODE" = "host" ]; then
    echo "=== Launching Tailscale KVM Host Controller ==="
    exec "$HOST_BIN" "$@"
fi

# Default: all / both
echo "========================================================"
echo "         Tailscale Universal KVM Unified Node          "
echo "  Any computer running this can be a Host or Receiver!  "
echo "========================================================"
echo

echo "[1/2] Starting Background Receiver Daemon (Port 9876)..."
"$RECV_BIN" "$@" &
RECV_PID=$!

cleanup() {
    echo
    echo "Shutting down KVM Receiver daemon (PID: $RECV_PID)..."
    kill "$RECV_PID" 2>/dev/null || true
    pkill -f "tailscale-kvm-receiver" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

echo "[2/2] Starting Host Controller & Web GUI (:9877)..."
echo
"$HOST_BIN" "$@"
