# Area 4: themes and display settings

## Goal

The web app's themes (light, dark, black, sepia, and the others in `apps/web/src/styles.css` `:root[data-theme=..]`),
following the system appearance by default, and the display settings: text size (small 15 / normal 18 / large 21px)
and density (compact / normal / comfortable row padding). `style.rs` holds the light colors as functions today: make
them come from the current theme, reactively. A settings screen (the top bar's gear on desktop, the menu on phones)
with these options; keep the choices on the device (NSH storage).

## Done when

Each theme matches the web app (structural snapshot and offscreen render, area 5), changing settings applies at once and survives a restart, headless
tests cover it, CI green, TASKS.md updated, committed and pushed.
