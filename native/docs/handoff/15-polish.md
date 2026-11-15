# Area 15: follow-ups the earlier areas left

Small items from the reports of areas 10-14 (TASKS.md section 4 lists most as "웹과 다른 점" / "아직 없는 것").
The web app is the reference unless a decision below says otherwise.

1. Web bugs found by the native areas (fix on the web, with a vitest test each):
   - Dragging a file row down within its folder drops it one place too far (`apps/web/src/ui/sidebar.ts`; the
     native `sidebar.rs` already corrects the index).
   - Decision (orchestrator, 2026-11-15): the item menu acts on the item it was opened on (or on the selected
     items when opened on a bullet inside the selection), not on the item being edited elsewhere: make the web
     do what the native item menu and hover toolbar do.
2. Native drag: auto-scroll while dragging items near the list's top or bottom; dropping items onto a document
   in the sidebar moves them there (as the web); item menu submenus open on hover on desktops.
3. View options per document as the web's (show checked, notes show / 1st line / hide, Article layout if the web
   has it), stored on the workspace file entry as the web stores them.
4. The file finder's second line shows the full folder path as the web's.
5. Search results' bookmarks reopen the flat / every-document results they were made from, as the web.
6. Anything else in TASKS.md section 4 marked as missing that is small and does not need the server or a real
   device; list what stays out and why.

Checks as before: headless tests on small fixtures (no sleeps), the structural snapshot and `qa:model` for new
looks, cargo test twice and clippy; web `npm test` / typecheck for the web fixes. Design values only through tokens.
The only session on the branch now; still `git pull --rebase` before each push.

## Done when

All of the above with tests, CI (ci.yml by workflow_dispatch, native.yml) green, TASKS.md updated, committed and
pushed, working tree clean.
