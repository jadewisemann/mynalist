#!/usr/bin/env bash
# Prove that a menu click reaches a web-app command.
#
# The macOS menu bar is the part of this shell this machine cannot build, so the
# risk is not "does it compile" but "does the keydown it sends actually match the
# web keymap". That *is* testable here: `forward_combo()` builds a synthetic
# keydown, and the web app either reacts or does not. This replays exactly that
# keydown into the real web build and checks for an observable effect.
#
# The modifier is chosen the way the web app chooses it: `IS_MAC` in
# apps/web/src/editor/keymap.ts is `/Mac|iPhone|iPad/.test(navigator.platform)`,
# so `Mod` is Cmd (metaKey) on macOS and Ctrl (ctrlKey) on Linux. This machine
# reports non-Mac, so the test must use ctrlKey -- and that is itself worth
# knowing, because it means the Linux build is not a faithful proxy for the
# macOS modifier. What it does prove is the dispatch mechanism and the combo
# strings, which are platform-independent.
#
#   Mod+,  -> opens the settings dialog
#   Mod+/  -> opens the shortcut help
#
# Usage: scripts/smoke-menu.sh
set -uo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${OUTLINER_BIN:-$HERE/src-tauri/target/debug/outliner}"
: "${DISPLAY:=:99}"
export DISPLAY

[ -x "$BIN" ] || { echo "FAIL: binary not found: $BIN"; exit 2; }

JS='(()=>{
  const isMac = /Mac|iPhone|iPad/.test(navigator.platform);
  const mod = isMac ? {metaKey:true} : {ctrlKey:true};
  const fire = (key, code) => (document.activeElement||document.body)
    .dispatchEvent(new KeyboardEvent("keydown", Object.assign({key, code, bubbles:true, cancelable:true}, mod)));
  const overlays = () => [...document.querySelectorAll(".overlay")];

  const before = overlays().length;
  fire(",", "Comma");
  const settings = overlays().length;
  const settingsText = overlays().map(o => o.innerText || "").join(" ");

  // Close it before the next probe so the counts stay independent.
  document.dispatchEvent(new KeyboardEvent("keydown", {key:"Escape", code:"Escape", bubbles:true, cancelable:true}));
  const closed = overlays().length;

  // The help panel is a persistent side panel (`.help-panel`), not an overlay,
  // so it is detected by its own element and `hidden` attribute.
  fire("/", "Slash");
  const helpEl = document.querySelector(".help-panel");
  const help = helpEl && !helpEl.hidden ? 1 : 0;
  const helpText = helpEl ? (helpEl.innerText || "") : "";


  return JSON.stringify({
    isMac, before, settings, closed, help,
    settingsOk: /Settings/.test(settingsText),
    helpOk: /Shortcuts|Formatting|Keymap/.test(helpText),
  });
})()'

LOG="$(mktemp /tmp/outliner-menu.XXXXXX.log)"
OUTLINER_SMOKE_EVAL="$JS" OUTLINER_SMOKE_DELAY=4 \
  timeout 20 dbus-run-session -- "$BIN" >"$LOG" 2>&1

RESULT="$(grep -m1 'outliner: eval' "$LOG" | sed 's/^outliner: eval //')"
echo "eval: ${RESULT:-<none>}"
echo "--- stderr ---"; grep -E "outliner:|panic|CRITICAL" "$LOG" || true
echo "--------------"

# The payload is Rust's Debug form of a JSON string, so quotes arrive escaped.
has() { case "$RESULT" in *"\\\"$1\\\":$2"*|*"\"$1\":$2"*) return 0;; *) return 1;; esac; }

RC=0
if [ -z "$RESULT" ]; then
  echo "FAIL: webview evaluate returned nothing"; RC=1
else
  has settings 1 && echo "OK: Mod+, opened the settings dialog" \
                  || { echo "FAIL: Mod+, did not open settings"; RC=1; }
  has settingsOk true && echo "OK: the dialog is the settings dialog" \
                       || { echo "FAIL: overlay text is not settings"; RC=1; }
  has closed 0 && echo "OK: Escape closed it (the overlay teardown path works)" \
               || { echo "WARN: Escape left an overlay open (count: $(sed -n 's/.*"closed":\([0-9]*\).*/\1/p' <<<"$RESULT"))"; }
  has help 1 && echo "OK: Mod+/ opened the shortcuts panel" \
             || { echo "FAIL: Mod+/ did not open the shortcuts panel"; RC=1; }
  has helpOk true && echo "OK: the panel lists shortcuts and formatting" \
                  || { echo "FAIL: shortcuts panel text unexpected"; RC=1; }
fi

[ "$RC" -eq 0 ] && echo "PASS" || echo "FAIL"
exit "$RC"
