# Area 6: one design source for the web app and the native app

The behavior is kept equal by a spec and shared tests (the TS core is the oracle for the Rust port). Do the same for
the look: one source of values, one spec of relations, one measured model both apps are checked against. Comes after
area 5 (whose structural snapshot is already in the shared render-model shape) and before areas 2-4.

## 1. Design tokens with modes

`design/tokens.json` (repo root; W3C design-tokens style or a plain JSON) holds every value the look uses: colors,
type (families, sizes, line heights, weights), spacing and metrics (row padding, indent, bullet, checkbox, top bar,
phone toolbar, sidebar), radii. Values vary by **modes**: theme (all themes of `apps/web/src/styles.css`), font size
(small / medium / large), density (compact / cozy / comfortable, `docs/03-features-ui.md` §2.3), font family
choices. A generator (Node script, or Style Dictionary) writes:

- a CSS file of custom properties per mode for the web app, which `styles.css` uses instead of its own `:root` blocks;
- a Rust module (`outliner-app`) with the same values per mode, which `style.rs` reads.

Generated files are committed and a CI step fails if they are stale. The web app must look exactly as before: its
QA (`npm run qa:serve && npm run qa`, including `layout` and `layout --mobile`) must still pass.

## 2. Spec of relations

`design/spec.json`: the relations tokens alone do not say (row pitch, bullet center to text left, title offset,
indent per level, note gap, checkbox to text, header heights ...), with expected values per mode and tolerances.
Start from the metrics `qa/layout.mjs` measures (`qa/golden/layout*.json`).

## 3. Shared render model, measured on both

The structural snapshot's model from area 5 (rows: depth, bullet center x, text left, top, height, spans with
family / size / weight / italic / color / decoration / background; title; top bar; toolbar). Write a web measurer
(Playwright: `getBoundingClientRect`, `getComputedStyle`; next to `qa/layout.mjs`) that produces the same model
from the same fixtures. Tests:

- each app's model satisfies `spec.json`, for a chosen set of mode combinations (all themes at medium/cozy, all
  sizes x densities in the light theme, not the full product);
- the web model and the native model of the same fixture and modes are equal within tolerance (fonts' glyph widths
  differ between engines: compare positions that do not depend on text width exactly, widths loosely).

## 4. Custom CSS (the web's Settings > Custom CSS, pref `custom_css`)

The native app has no CSS engine. Define the subset both apps honor: custom-property overrides on `:root`
(`--tag: teal`, i.e. token overrides) and simple rules on the known classes (`.o-tag`, `.o-code`, `.o-link`,
`.o-content`, `.o-note`, `.o-h1`..`.o-h3`, `.o-checked`, `.o-bullet`, ...) with text properties (color, background,
font-weight, font-style, text-decoration, font-size). Parse it in Rust (a CSS parser crate), apply that subset, and
list what was ignored so the settings screen can say "web only". Test: the same snippets give the same render model
on both, for the supported part. The settings screen itself is area 4's.

## Done when

Tokens and spec are the single source (no color or metric literals left in `styles.css` or `style.rs` outside the
generated files), the web QA still passes, the cross-checks run in CI, custom-CSS subset tested, TASKS.md updated,
committed and pushed, working tree clean.
