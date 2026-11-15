# Area 13: the editing features still missing in the native app

The web app is the reference (`apps/web/src/editor/`, `apps/web/src/ui/itemmenu.ts`, `hoverbar.ts`, `popups.ts`,
`keymap.ts`; `docs/02-editor-keyboard.md`). TASKS.md section 4 lists these as "아직 없는 것" of areas 1, 9, 10, 11.

1. Item menu: the web's menu on a bullet (right click / long press; the hover toolbar on desktops when the
   `item_toolbar_hover` pref is on, then add that pref to the settings): color labels, highlight swatches, heading
   levels, checkbox / numbered list / board toggles the web has, copy link, duplicate, move to, export, delete.
2. Formatting keys: every command of `keymap.rs` in the Format group (Mod+B, Mod+I, Mod+U, code, strike,
   highlight, link, ...) and any other keymap command the native app lists but does not run yet.
3. Date chips: pressing a date chip opens the date picker on that date (popover under the chip on desktops).
4. Mouse selection on desktops: Shift+click and Mod+click to select items, dragging across rows to select; drag
   and drop of items (and of selected items) to move them, with the web's drop marks; "Move to…" (the web's
   `move_item_sent_to_position` pref) for phones and the menu.
5. The zoom root's note (the title's note) editable as on the web.
6. File menu "Import OPML…" (the `paste` port) where the web's menu has it.
7. Decision (orchestrator, 2026-11-15): with `workflowy_shortcuts` on, its Alt+Right/Left zoom keys win over
   Back/Forward on the same keys; fix this on the web (`apps/web/src/editor/keymap.ts`, with a test) and natively.

Checks as before: headless tests on small fixtures (no sleeps), the structural snapshot and `qa:model` for new
looks (menu, swatches, drop marks, selection), cargo test twice and clippy; web `npm test` / typecheck for the
web fix. Design values only through tokens. Runs in parallel with area 14: you own `outline.rs`, `commands.rs`,
`keymap.rs`, `datepicker.rs` and a new item-menu module; `git pull --rebase` before each push.

## Done when

All of the above with tests, CI (ci.yml by workflow_dispatch, native.yml) green, TASKS.md updated, committed and
pushed, working tree clean.
