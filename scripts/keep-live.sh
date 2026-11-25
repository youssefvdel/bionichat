#!/usr/bin/env bash
# Keep the bionichat stack alive: the CEF app on a display.
# Runs on the single shared agent desktop (:0, Selkies) — this box forbids
# extra X servers (agent-desktop-guard kills any Xvfb/Xorg that is not the
# agent desktop). Override with DISPLAY_NUM=<n> only for a short manual test.
# Safe to kill any time.
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG="/tmp/opencode/bionichat-live.log"
DISPLAY_NUM="${DISPLAY_NUM:-:0}"

# :0 is owned by the Selkies session and needs its per-run auth cookie.
if [[ "$DISPLAY_NUM" == ":0" && -z "${XAUTHORITY:-}" ]]; then
  XAUTHORITY="$(ls -d /run/user/"$(id -u)"/selkies-session-*/Xauthority 2>/dev/null | head -1)"
  export XAUTHORITY
fi
export DISPLAY="$DISPLAY_NUM"

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
