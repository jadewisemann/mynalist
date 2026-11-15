# Area 10: search, links, paste and export in the native app

Connect the core ports of area 7 (`outliner_core::{search, refs, paste, export, render}`, see TASKS.md section 4
"남은 core 이식" for their entry points) to the app. The web app is the reference: `apps/web/src/ui/results.ts`,
`replace.ts`, `popups.ts` (suggestions: `[[` links, `#tags`, `@`), `exporter.ts`, and the editor's search /
paste code under `apps/web/src/editor/`; `docs/` for the specs.

1. Search: the top bar's search button (desktop and phone) opens the web's search box; typing filters the
   outline as the web does (`search::compile_query`, `search_tree`, the visible set as the view's filter, the
   matched words highlighted in rows with `inline::match_ranges`), Escape / the close button ends it. The search
   tips popup and the results list across documents if the web has them for the open document set.
2. Find and replace (`refs::count_matches`, `replace_all`), as the web's dialog, undoable in one step.
3. Item links: typing `[[` suggests items (`search::fuzzy_filter`, the web's candidates and order), choosing one
   inserts the web's link markup; pressing a link to an item zooms to it (`refs::parse_item_url`); backlinks under
   the zoomed item when the `show_backlinks` pref is on (`refs::find_backlinks`, counts as the web shows them).
   `#` tag suggestions from the open document.
4. Paste: Mod+V / the platform paste in a row input with several lines, HTML or OPML on the clipboard becomes items
   (`paste::parse_clipboard` → `commands::paste_items`), one line stays plain text in the input, as the web.
5. Export: the web's export dialog (plain text / HTML / OPML, visible only or all) for the document, the zoomed
   item or the selected items; desktop saves with the save dialog or copies, phones copy (no share sheet in NSH
   0.5.0).

Checks as before: headless tests on small fixture documents (no sleeps; `wait_until`), the structural snapshot and
`qa:model` for what changes the look (search box, highlighted matches, suggestions list, backlinks), cargo test
twice and clippy. Design values only through tokens (`design/*.toml`, add entries the web uses as literals there,
web unchanged). Runs in parallel with areas 11 and 12: you own the outline, the top bar's search and new modules
for search/links/export; touch `store.rs` / `commands.rs` / `sidebar.rs` / `settings*` only where needed, small, and
`git pull --rebase` before each push.

## Done when

All of the above with tests, CI (ci.yml by workflow_dispatch, native.yml) green, TASKS.md updated, committed and
pushed, working tree clean.
