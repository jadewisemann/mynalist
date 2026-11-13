#!/usr/bin/env bash
# Headless smoke test for the desktop shell (Linux).
#
# This machine has no GPU and runs under Xvfb, so pixel capture is not usable as
# evidence: a GPU-less Xvfb screenshot comes out black. Pass/fail is decided by
# signals that are honest in a headless environment:
#
#   1. the process starts, creates a window, and keeps it
#   2. stderr has the shell's own "shell ready" line and no fatal GTK/WebKit errors
#   3. the *web build actually booted inside the webview*, verified by asking the
#      page for `document.title` and its rendered text. The outliner sets the
#      title to "<doc> — Outliner" and prints the welcome document, so an
#      empty/failed bundle cannot pass.
#
# Usage: scripts/smoke-linux.sh
#   DISPLAY must point at an X server (default :99).
#   A dbus session is required: WebKitGTK waits for xdg-desktop-portal at
#   startup, which needs a session bus. `dbus-run-session` provides one.
set -uo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${OUTLINER_BIN:-$HERE/src-tauri/target/debug/outliner}"
: "${DISPLAY:=:99}"
export DISPLAY
WAIT="${WAIT:-9}"

if [ ! -x "$BIN" ]; then
  echo "FAIL: binary not found: $BIN"
  echo "  build it: cargo build --manifest-path src-tauri/Cargo.toml"
  exit 2
fi

LOG="$(mktemp /tmp/outliner-smoke.XXXXXX.log)"
RC=0

# The eval hook is compiled into the shell on purpose: it is how a machine with
# no GPU can prove the webview rendered something (see src-tauri/src/lib.rs).
OUTLINER_SMOKE_EVAL='JSON.stringify({title:document.title,app:!!document.querySelector("#app .app"),text:(document.querySelector("#app")||{}).innerText||""})' \
OUTLINER_SMOKE_DELAY=5 \
  timeout "$((WAIT + 8))" dbus-run-session -- "$BIN" >"$LOG" 2>&1 &
PID=$!
sleep "$WAIT"

# Tauri creates one real window plus a 1x1/10x10 helper window, so pick the
# largest window that carries the product name.
pick_window() {
  local best="" best_area=0 w geo wh area
  for w in $(xdotool search --name 'Outliner|outliner' 2>/dev/null); do
    geo="$(xdotool getwindowgeometry --shell "$w" 2>/dev/null)" || continue
    wh="$(sed -n 's/^WIDTH=\([0-9]*\)$/\1/p' <<<"$geo")x$(sed -n 's/^HEIGHT=\([0-9]*\)$/\1/p' <<<"$geo")"
    # shellcheck disable=SC2086
    area=$(( ${wh%x*} * ${wh#*x} ))
    if [ "$area" -gt "$best_area" ]; then best="$w"; best_area="$area"; fi
  done
  echo "$best"
}
WINID="$(pick_window)"
NAME="$( [ -n "$WINID" ] && xdotool getwindowname "$WINID" 2>/dev/null )"
GEO="$( [ -n "$WINID" ] && xdotool getwindowgeometry --shell "$WINID" 2>/dev/null | tr '\n' ' ' )"
EVAL="$(grep -m1 'outliner: eval' "$LOG" | sed 's/^outliner: eval //')"

kill "$PID" 2>/dev/null
wait "$PID" 2>/dev/null

echo "binary:  $BIN"
echo "window:  ${WINID:-<none>} ${NAME:-<no name>}"
echo "geometry: ${GEO:-<unknown>}"
echo "webview: ${EVAL:-<no eval result>}"
echo "--- relevant stderr ---"
grep -E "outliner:|panic|CRITICAL|segmentation|WebKitWebProcess" "$LOG" || true
echo "-----------------------"

# 1. window exists and has the configured size
if [ -z "$WINID" ]; then
  echo "FAIL: no X11 window titled 'Outliner'"; RC=1
elif ! grep -q 'WIDTH=1200' <<<"$GEO"; then
  echo "WARN: window geometry is not the configured 1200x820"
fi

# 2. no fatal log lines
if grep -qiE 'panicked|segmentation fault|Gtk-CRITICAL|WebKitWebProcess.*(crashed|terminated)' "$LOG"; then
  echo "FAIL: fatal lines in log"; RC=1
fi

# 3. the web build booted inside the webview
case "$EVAL" in
  *'Welcome — Outliner'*) echo "OK: web bundle booted (document.title set by the app)" ;;
  '') echo "FAIL: webview evaluate returned nothing (bundle did not run)"; RC=1 ;;
  *) echo "WARN: unexpected eval payload: $EVAL" ;;
esac
# The eval payload is a JSON string escaped by Rust's Debug format, so match the
# escaped form rather than plain `"app":true`.
case "$EVAL" in
  *'"app":true'*|*'\"app\":true'*) echo "OK: app root mounted" ;;
  *) echo "FAIL: '#app .app' not found"; RC=1 ;;
esac

if [ "$RC" -eq 0 ]; then
  echo "PASS"
  echo "(negative control: deleting dist/index.html must fail this test)"
else
  echo "FAIL"
fi
exit "$RC"
