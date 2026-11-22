#!/usr/bin/env bash
# Keep the bionichat stack alive: the CEF app on a display.
# Defaults to an isolated Xvfb (:98) so nothing ever appears on the real
# desktop; DISPLAY_NUM=:0 puts it on the desktop instead.
# Safe to kill any time.
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG="/tmp/opencode/bionichat-live.log"
DISPLAY_NUM="${DISPLAY_NUM:-:98}"

log() { echo "[$(date -u +%H:%M:%S)] keep-live: $*" >>"$LOG"; }

# Detach fully so tool sessions ending cannot take us down.
if [[ -z "${KEPT_ALIVE:-}" ]]; then
  export KEPT_ALIVE=1
  setsid nohup "$0" "$@" </dev/null >>"$LOG" 2>&1 &
  exit 0
fi

log "supervisor started (pid $$)"
while true; do
  if [[ "$DISPLAY_NUM" != ":0" ]] && ! pgrep -f "Xvfb $DISPLAY_NUM" >/dev/null; then
    log "Xvfb $DISPLAY_NUM missing — starting"
    Xvfb "$DISPLAY_NUM" -screen 0 1400x900x24 >>"$LOG" 2>&1 &
    sleep 2
  fi
  if ! pgrep -x bionichat >/dev/null; then
    log "bionichat missing — starting on $DISPLAY_NUM"
    DISPLAY="$DISPLAY_NUM" "$ROOT/scripts/run-cef.sh" >>"$LOG" 2>&1 &
    sleep 8
  fi
  sleep 15
done
