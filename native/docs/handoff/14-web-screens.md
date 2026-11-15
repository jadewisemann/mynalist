# Area 14: the web's other screens in the native app

The web app is the reference: `apps/web/src/ui/help.ts` (help panel and keyboard shortcuts), `tour.ts` (first-run
tour), `mindmap.ts` (mind map view), `inbox.ts` (inbox destination and "send to inbox"), `history.ts` (version
history), `share.ts`, plus the read mode (top bar lock button) and any top bar / phone toolbar button that is still
drawn but does nothing (`toolbar.rs`: Forward, Read mode, "Move item to").

1. Read mode (lock button): the document can't be edited while it is on, as on the web.
2. Back/Forward through zoom history on desktops (the top bar's buttons), as the web.
3. Help panel with the keyboard shortcuts (from `keymap.rs`), and the first-run tour if the web shows one locally.
4. Mind map view of the zoomed item, as the web's (read-only if the web's is).
5. Inbox: the inbox destination pref (`Your inbox` section in settings) and "send to inbox" as the web does it
   locally (quick capture into the destination item).
6. Version history and Share depend on the server: leave them out, list them in TASKS.md with sync.
7. Every remaining "draws but does nothing" button gets its feature or is removed where the web has none.

Checks as before: headless tests on small fixtures (no sleeps), the structural snapshot and `qa:model` for new
looks, cargo test twice and clippy. Design values only through tokens. Runs in parallel with area 13: you own
`toolbar.rs`, new modules for these screens, and the settings rows they need; touch `outline.rs` / `commands.rs`
only where needed, small, and `git pull --rebase` before each push.

## Done when

All of the above with tests, CI (ci.yml by workflow_dispatch, native.yml) green, TASKS.md updated, committed and
pushed, working tree clean.
