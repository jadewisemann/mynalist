# Area 12: the remaining settings, and date picker follow-ups

The web's settings dialog is the reference (`apps/web/src/ui/settings.ts`): area 4 made Preferences (theme,
density, font size, fonts, true black) and Look. Add the rest the native app can honor, with the web's pref names,
defaults and storage as area 4 did.

1. Preferences: the web's other sections (Appearance toggles: current highlight, bullet threading, narrow layout,
   right-to-left if NSH can; Viewing: show checked, notes show/1st line/hide; Input: word count, tag autocomplete
   from all docs, WorkFlowy-style shortcuts; Date: time format 12h/24h, first day of week, display formats, encode
   timezone, highlight overdue, recurring behavior; Internal links: show backlinks; Control: bullet to zoom,
   unindent in place, ...; the Advanced block). Each one must change the native app's behavior or look as on the
   web; leave out (and list in TASKS.md) what has no native meaning yet (sync, inbox until there is one).
2. Keymap tab: the web's keymap editor (`keymapEditor`, `apps/web/src/editor/keymap.ts` commands and defaults) and
   the native key bindings reading it on desktops.
3. Sync & data tab: Backup (a zip of every document as text and OPML, `export` port; save dialog on desktops,
   phones as NSH allows) and Import (OPML / text files, `paste` port). The sync server part waits for sync: show
   it as the web does when there is no server, nothing more.
4. Date picker decisions (orchestrator, 2026-11-15): on desktops open it as a popover under the row being edited,
   at the caret's or text's left as the web's (`apps/web/src/ui/position.ts`), instead of centered; phones keep
   the bottom sheet. Week start and 12h/24h come from the prefs of item 1 (the picker, date chips and the
   calendar).
5. Web bug (fix on the web, with a test): in `apps/web/src/ui/datepicker.ts`, after PageUp/Down on a date that is
   not a range, Done turns the old end into the start (`done`'s swap); move the end with the start as the native
   picker does.

Checks as before: headless tests (`tests/settings.rs`, small fixtures, no sleeps), the structural snapshot and
`qa:model` for the new settings screens and any pref that changes the look, `npm test` / typecheck for the web
fix, cargo test twice and clippy. Design values only through tokens. Runs in parallel with areas 10 and 11: you
own `settings*`, `datepicker.rs`, `appearance`/prefs code and the web fix; other files only where a pref needs it,
small, and `git pull --rebase` before each push.

## Done when

All of the above with tests, CI (ci.yml by workflow_dispatch, native.yml) green, TASKS.md updated, committed and
pushed, working tree clean.
