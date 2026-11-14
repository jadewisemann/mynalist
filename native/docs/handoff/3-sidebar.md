# Area 3: sidebar

> On hold (the user, 2026-11-14): not to be started until the user asks.

## Goal

The web app's sidebar (`apps/web/src/ui/sidebar.ts`, `.sidebar` styles): Files / Bookmarks / Tags tabs, the file
tree, on desktop a resizable side panel (260px default) toggled by the top bar's sidebar button, on phones a drawer
(82vw, max 320px) over a dimmed scrim. Until sync exists the native app has only local documents: give the store
several documents (the demo one plus an empty one is enough), switching between them, creating and renaming. The
Tags tab lists the tags found in the open document (`extractTags`, from the inline port of area 1).

## Done when

Desktop and phone layouts match the web app's screenshots, switching documents works, headless tests cover it, CI
green, TASKS.md updated, committed and pushed.
