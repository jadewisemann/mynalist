# Area 2: current-row thread and interaction marks

> On hold (the user, 2026-11-14): not to be started until the user asks.

## Goal

The web app marks the row being edited and its ancestors: the bullets on that path are blue (`--accent`) and a
2px blue "thread" runs from the root's bullet down through the guides to the current row (`.o-thread`, `.o-path`,
`.o-current`, `.o-above` in `apps/web/src/styles.css`, logic in `apps/web/src/editor/`). Match it in the native
app, plus the bullet hover marks on desktop (triangle down / right on items with children) and the selection color.

## Done when

Screenshots match the web app with a row being edited at depth 2 or more, headless tests still pass, CI green,
TASKS.md updated, committed and pushed.
