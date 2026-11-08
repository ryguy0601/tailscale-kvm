#!/usr/bin/env bash
set -euo pipefail

# Tailscale KVM Dual-PC Demo Recorder & Side-by-Side Stitcher
#
# Usage:
#   1) Trigger synchronized live recording across Host & Receiver (via Host GUI API :9877):
#        ./scripts/stitch-demo.sh [--slot right|left|up|down] [--duration 10] [--url http://localhost:9877]
#
#   2) Stitch any two existing Host & Receiver recordings side-by-side:
#        ./scripts/stitch-demo.sh --host host.mp4 --recv receiver.mp4 [--layout right|left|up|down] \
#          [--host-label "WINDOWS HOST"] [--recv-label "LINUX RECEIVER"] [--out demoPics/02-dual-screen-demo.webp]

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOST_CLIP=""
RECV_CLIP=""
LAYOUT="right"
DURATION="0"
API_URL="http://127.0.0.1:9877"
HOST_LABEL="HOST"
RECV_LABEL="RECEIVER"
OUT_WEBP="$REPO_DIR/demoPics/02-dual-screen-demo.webp"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --host) HOST_CLIP="$2"; shift 2 ;;
    --recv|--receiver) RECV_CLIP="$2"; shift 2 ;;
    --layout|--slot) LAYOUT="$2"; shift 2 ;;
    --duration) DURATION="$2"; shift 2 ;;
    --url) API_URL="$2"; shift 2 ;;
    --host-label) HOST_LABEL="$2"; shift 2 ;;
    --recv-label) RECV_LABEL="$2"; shift 2 ;;
    --out) OUT_WEBP="$2"; shift 2 ;;
    -h|--help)
      sed -n '3,14p' "$0"
      exit 0
      ;;
    *)
      echo "[!] Unknown option: $1" >&2
      exit 1
      ;;
  esac
done

escape_dt() {
  printf '%s' "$1" | sed "s/\\\\/\\\\\\\\/g; s/:/\\\\:/g; s/'/\\\\'/g"
}

if [[ -n "$HOST_CLIP" && -n "$RECV_CLIP" ]]; then
  command -v ffmpeg >/dev/null 2>&1 || { echo "[!] ffmpeg is required for offline stitching." >&2; exit 1; }
  mkdir -p "$(dirname "$OUT_WEBP")"
  OUT_MP4="${OUT_WEBP%.webp}.mp4"
  OUT_GIF="${OUT_WEBP%.webp}.gif"

  case "${LAYOUT,,}" in
    left)
      FIRST_IN="$RECV_CLIP"; SECOND_IN="$HOST_CLIP"
      FIRST_LBL="$(escape_dt "$RECV_LABEL")"; SECOND_LBL="$(escape_dt "$HOST_LABEL")"
      STACK="hstack=inputs=2:shortest=1"
      PAD="[v0]pad=iw+6:ih:0:0:color=#00d4be@0.35[v0p];"
      PW=720; PH=450
      ;;
    up|top)
      FIRST_IN="$RECV_CLIP"; SECOND_IN="$HOST_CLIP"
      FIRST_LBL="$(escape_dt "$RECV_LABEL")"; SECOND_LBL="$(escape_dt "$HOST_LABEL")"
      STACK="vstack=inputs=2:shortest=1"
      PAD="[v0]pad=iw:ih+6:0:0:color=#00d4be@0.35[v0p];"
      PW=960; PH=450
      ;;
    down|bottom)
      FIRST_IN="$HOST_CLIP"; SECOND_IN="$RECV_CLIP"
      FIRST_LBL="$(escape_dt "$HOST_LABEL")"; SECOND_LBL="$(escape_dt "$RECV_LABEL")"
      STACK="vstack=inputs=2:shortest=1"
      PAD="[v0]pad=iw:ih+6:0:0:color=#00d4be@0.35[v0p];"
      PW=960; PH=450
      ;;
    *)
      FIRST_IN="$HOST_CLIP"; SECOND_IN="$RECV_CLIP"
      FIRST_LBL="$(escape_dt "$HOST_LABEL")"; SECOND_LBL="$(escape_dt "$RECV_LABEL")"
      STACK="hstack=inputs=2:shortest=1"
      PAD="[v0]pad=iw+6:ih:0:0:color=#00d4be@0.35[v0p];"
      PW=720; PH=450
      ;;
  esac

  make_filter() {
    local fps="$1" w="$2" h="$3" draw="$4"
    local p0="[0:v]setpts=PTS-STARTPTS,fps=${fps},scale=${w}:${h}:force_original_aspect_ratio=decrease,pad=${w}:${h}:(ow-iw)/2:(oh-ih)/2:color=#0b0f17,setsar=1"
    local p1="[1:v]setpts=PTS-STARTPTS,fps=${fps},scale=${w}:${h}:force_original_aspect_ratio=decrease,pad=${w}:${h}:(ow-iw)/2:(oh-ih)/2:color=#0b0f17,setsar=1"
    if [[ "$draw" == "1" ]]; then
      echo "${p0},drawtext=text='${FIRST_LBL}':x=14:y=12:fontsize=18:fontcolor=#f0f6fc:box=1:boxcolor=#0b0f17@0.82:boxborderw=7[v0];${p1},drawtext=text='${SECOND_LBL}':x=14:y=12:fontsize=18:fontcolor=#00d4be:box=1:boxcolor=#0b0f17@0.82:boxborderw=7[v1];${PAD}[v0p][v1]${STACK}[outv]"
    else
      echo "${p0}[v0];${p1}[v1];${PAD}[v0p][v1]${STACK}[outv]"
    fi
  }

  echo "[*] Stitching $FIRST_IN + $SECOND_IN ($LAYOUT layout) -> $OUT_WEBP ..."
  FLT_WEBP="$(make_filter 15 "$PW" "$PH" 1)"
  if ! ffmpeg -y -i "$FIRST_IN" -i "$SECOND_IN" -filter_complex "$FLT_WEBP" -map "[outv]" \
      -c:v libwebp_anim -lossless 0 -compression_level 4 -q:v 78 -loop 0 -preset default -an "$OUT_WEBP" 2>/dev/null; then
    FLT_WEBP_PLAIN="$(make_filter 15 "$PW" "$PH" 0)"
    ffmpeg -y -i "$FIRST_IN" -i "$SECOND_IN" -filter_complex "$FLT_WEBP_PLAIN" -map "[outv]" \
      -c:v libwebp_anim -lossless 0 -compression_level 4 -q:v 78 -loop 0 -an "$OUT_WEBP"
  fi

  FLT_MP4="$(make_filter 24 "$PW" "$PH" 1)"
  ffmpeg -y -i "$FIRST_IN" -i "$SECOND_IN" -filter_complex "$FLT_MP4" -map "[outv]" \
    -c:v libx264 -preset veryfast -crf 22 -pix_fmt yuv420p -an "$OUT_MP4" 2>/dev/null || true

  FLT_GIF="$(make_filter 12 600 376 1);[outv]split[s0][s1];[s0]palettegen=max_colors=192:stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=3[gifv]"
  ffmpeg -y -i "$FIRST_IN" -i "$SECOND_IN" -filter_complex "$FLT_GIF" -map "[gifv]" -loop 0 -an "$OUT_GIF" 2>/dev/null || true

  echo "[+] Exported dual-PC demo assets:"
  ls -lh "$OUT_WEBP" "$OUT_GIF" "$OUT_MP4" 2>/dev/null || true
  exit 0
fi

# Live Synchronized API Mode
echo "[*] Triggering synchronized Dual-PC capture via ${API_URL}/api/record/start ..."
START_RESP="$(curl -fsS -X POST "${API_URL}/api/record/start" \
  -H "Content-Type: application/json" \
  -d "{\"slot\":\"${LAYOUT}\",\"auto_export\":true}")" || {
  echo "[!] Could not start recording via Host API (${API_URL})." >&2
  echo "    Ensure tailscale-kvm-host is running with an active Receiver slot, or pass --host <clip> --recv <clip>." >&2
  exit 1
}
echo "$START_RESP"

if [[ "$DURATION" =~ ^[0-9]+$ ]] && [[ "$DURATION" -gt 0 ]]; then
  echo "[*] Recording both Host & Receiver for ${DURATION}s..."
  sleep "$DURATION"
else
  read -r -p "[*] Recording BOTH computers live! Press [Enter] to stop & stitch... "
fi

echo "[*] Stopping captures and stitching..."
curl -fsS -X POST "${API_URL}/api/record/stop" >/dev/null

for _ in $(seq 1 60); do
  ST_JSON="$(curl -fsS "${API_URL}/api/record/status")"
  STATUS="$(printf '%s' "$ST_JSON" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))")"
  if [[ "$STATUS" == "ready" || "$STATUS" == "error" ]]; then
    echo "$ST_JSON" | python3 -m json.tool
    break
  fi
  sleep 1
done
