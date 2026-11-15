# Area 9: editing features of the native app

The web app's behavior is the reference (`apps/web/src/editor/`, keymap `apps/web/src/editor/keymap.ts`,
`docs/02-editor-keyboard.md`).

1. Notes: edit the item's note (prop `n`) in its own input under the content; Shift+Enter moves between content
   and note (the web's `goto-note`), Up/Down and Backspace at the note's edges as the web does; the phone toolbar's
   note button. Empty notes take no line.
2. Block selection: Shift+Up/Down, Mod+Shift+A, Escape, and the commands on a block (indent, outdent, move,
   delete, check, toggles, copy as text) using `outliner_core::commands` with `Sel::Block`; the phone's "Select
   multiple" mode from the top bar.
3. The date picker: inserting and editing a date (the web's `apps/web/src/ui/datepicker.ts`), from the phone
   toolbar's date button and Mod+D or the web's key.
4. Decisions delegated to the orchestrator (2026-11-15) about the Look tab of area 4: keep Apply, and also apply
   valid TOML when the text input loses focus; on phones export through the platform's share sheet if NSH has one
   (else keep the clipboard); move the caret to the error's line when NSH lets the app set the selection.
5. Anything editing-related area 4 / 3 left in TASKS.md that is small.

Checks as before: headless tests (small fixture documents; no sleeps), the structural snapshot and `qa:model`
for what changes the look (notes, the selection color), cargo test twice under load and clippy. Runs in parallel
with areas 7 and 8: only `crates/outliner-app` (and design tokens if needed) change; `git pull --rebase` before
each push.

## Done when

All of the above with tests, CI green, TASKS.md updated, committed and pushed, working tree clean.
