# Area 8: web fixes decided by the orchestrator

Decisions the user delegated (2026-11-15):

1. Stored `custom_css` values are kept and ignored (never deleted: it is user data). The web settings dialog says,
   where Custom CSS used to be, that it is no longer supported and points to the Look tab.
2. `==highlight==` text takes the body color (today the browser's `<mark>` default makes it black, also in dark
   themes). Fix it in the web styles through the tokens; remove the matching entry from `qa/lib/model.mjs` KNOWN
   once web and native agree.
3. Move every remaining px literal of `apps/web/src/styles.css` (web-only screens: dialogs, menus, settings, date
   picker, version history, mind map, ...) into tokens of `design/` (a spacing scale plus named sizes where a
   value is not on the scale). The web must look exactly as before except for point 2: the web QA
   (`npm run qa:serve && npm run qa`, layout and mobile included), `npm test`, `npm run qa:model` pass, and compare
   computed styles before / after as area 6 did.

Only `apps/web`, `design/` and `qa/` change (plus generated files). Runs in parallel with other sessions: commit
small, `git pull --rebase` before each push; if the generated Rust module changes, run the native tests too.

## Done when

No px literal left in `styles.css` outside generated files (or a short list of justified exceptions in the commit
message), the three points done, all checks above and CI green, TASKS.md updated, committed and pushed.
