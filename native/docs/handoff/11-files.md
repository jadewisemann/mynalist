# Area 11: files, folders and bookmarks in the native app

The web app's sidebar is the reference (`apps/web/src/ui/sidebar.ts`, `.sidebar` styles in `styles.css`, the
store under `apps/web/src/store/`). Area 3 made the panel/drawer, local documents (switch, create, rename) and the
Tags tab; this area finishes the sidebar except sync (no server yet: everything stays local, kept on the device).

1. Documents persist on the device (NSH storage or files; the web's local document format if it has one), so the
   app reopens with the user's documents instead of only the demo.
2. Folders: create, rename, nest, fold/unfold, as the web's file tree; the web's file menu on a file or folder
   (rename, duplicate, move, delete with the web's confirmation, ...) and the "New…" menu (document, folder).
3. Drag to reorder and move files into folders (desktop drag; on phones the web's equivalent, or long press then
   "Move to"), as far as NSH 0.5.0 allows; say in TASKS.md what it cannot do.
4. Bookmarks tab: bookmark the zoomed item or the document as the web does, list, open, remove.
5. Persist the sidebar's settings as the web does (open/closed, active tab, width, sort order) under the web's pref
   names. The items area 3 left in TASKS.md (double click to rename via long press, drawer animation, sort, width)
   where NSH allows.

Checks as before: headless tests on small fixtures (no sleeps), the structural snapshot and `qa:model` for new
looks (folders, menus, bookmarks list), cargo test twice and clippy. Design values only through tokens. Runs in
parallel with areas 10 and 12: you own `sidebar.rs`, the document storage and new modules for menus/bookmarks;
keep `store.rs` changes to the document list, small, and `git pull --rebase` before each push.

## Done when

All of the above with tests, CI (ci.yml by workflow_dispatch, native.yml) green, TASKS.md updated, committed and
pushed, working tree clean.
