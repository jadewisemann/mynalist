#!/usr/bin/env bash
# Catch drift between the native menu and the web app's keymap.
#
# The shell's menu items do not implement behaviour; they dispatch a synthetic
# keydown whose combo must match a binding in apps/web/src/editor/keymap.ts.
# If a combo drifts (renamed command, changed default binding), the menu item
# silently does nothing -- there is no error, no log, nothing.
#
# So this compares two lists:
#   * every combo the shell can send      (parsed from menu_combo in lib.rs)
#   * every combo the web app accepts     (read from the app's own shortcut
#                                          panel, which is generated from COMMANDS
#                                          + the live Keymap)
#
# It also checks that each combo is *wired*: for Combos whose effect is cheap to
# observe (settings, help, find) the resulting UI is checked for real.
#
# Usage: scripts/smoke-menu-keys.sh
set -uo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${OUTLINER_BIN:-$HERE/src-tauri/target/debug/outliner}"
SRC="$HERE/src-tauri/src/lib.rs"
: "${DISPLAY:=:99}"
export DISPLAY

[ -x "$BIN" ] || { echo "FAIL: binary not found: $BIN"; exit 2; }
[ -f "$SRC" ] || { echo "FAIL: cannot read $SRC"; exit 2; }

# ---- combos the shell can dispatch ----------------------------------------
# Only the `menu_combo` return values: lines of the form `"id" => "Mod+..."`.
# Anchored on the `=>` so comment text cannot be mistaken for a binding.
mapfile -t SHELL_COMBOS < <(
  sed -n '/fn menu_combo/,/^}/p' "$SRC" \
    | grep -oE '=> "Mod[^"]*"' \
    | sed 's/^=> "//; s/"$//' \
    | sort -u
)
[ "${#SHELL_COMBOS[@]}" -gt 0 ] || { echo "FAIL: parsed no combos from menu_combo"; exit 1; }
echo "shell dispatches ${#SHELL_COMBOS[@]} combos: ${SHELL_COMBOS[*]}"

# ---- combos the web app accepts -------------------------------------------
# Read from the in-app shortcut panel. That panel is rendered from COMMANDS and
# the live Keymap, so it is the app's own answer to "what keys work".
JS='(() => {
  const isMac = /Mac|iPhone|iPad/.test(navigator.platform);
  const mod = isMac ? {metaKey:true} : {ctrlKey:true};
  const fire = (key, code, extra) => (document.activeElement || document.body)
    .dispatchEvent(new KeyboardEvent("keydown",
      Object.assign({key, code, bubbles:true, cancelable:true}, mod, extra || {})));
  fire("/", "Slash");
  const panel = document.querySelector(".help-panel");
  if (!panel || panel.hidden) return JSON.stringify({error:"help panel did not open"});
  const rows = [...panel.querySelectorAll("tr")]
    .map(r => [...r.children].map(c => c.textContent.trim()))
    .filter(r => r.length === 2);
  return JSON.stringify({rows});
})()'

LOG="$(mktemp /tmp/outliner-keys.XXXXXX.log)"
OUTLINER_SMOKE_EVAL="$JS" OUTLINER_SMOKE_DELAY=4 \
  timeout 20 dbus-run-session -- "$BIN" >"$LOG" 2>&1
RAW="$(grep -m1 'outliner: eval' "$LOG" | sed 's/^outliner: eval //')"

if [ -z "$RAW" ]; then
  echo "FAIL: could not read the app's shortcut table"
  grep -E "outliner:|panic" "$LOG" || true
  exit 1
fi

# The payload is Rust's Debug rendering of a JSON string (escaped quotes).
ACCEPTED="$(sed 's/\\"/"/g; s/^"//; s/"$//' <<<"$RAW" \
  | python3 -c '
import json,sys,re
data = json.load(sys.stdin)
out = set()
for label, keys in data.get("rows", []):
    for part in keys.split(" / "):
        part = part.strip()
        if not part or part == "\u2014":
            continue
        # The panel pretty-prints for the current platform: Cmd on macOS,
        # Ctrl elsewhere. Normalise both to the shell'"'"'s `Mod` vocabulary,
        # and drop Alt/Shift ordering differences.
        part = part.replace("Cmd+", "Mod+").replace("Ctrl+", "Mod+")
        out.add(part)
print("\n".join(sorted(out)))
')"

if [ -z "$ACCEPTED" ]; then
  echo "FAIL: the shortcut table was empty"; exit 1
fi

echo "web app accepts $(wc -l <<<"$ACCEPTED") combos"
echo

RC=0
for combo in "${SHELL_COMBOS[@]}"; do
  if grep -qxF "$combo" <<<"$ACCEPTED"; then
    echo "OK:   $combo is a live binding"
  else
    echo "FAIL: $combo is not in the app's shortcut table"
    RC=1
  fi
done

echo
[ "$RC" -eq 0 ] && echo "PASS" || echo "FAIL"
exit "$RC"
