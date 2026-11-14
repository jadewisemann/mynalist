# Area 6: one design source for the web app and the native app

The behavior is kept equal by a spec and shared tests (the TS core is the oracle for the Rust port). Do the same for
the look: one source of values, one spec of relations, one measured model both apps are checked against. Comes after
area 5 (whose structural snapshot is already in the shared render-model shape) and before areas 2-4.

## 1. Design tokens with modes, in TOML

The user's decision: one shared config format, TOML, for the whole look of both apps; neither app styles itself
from hand-written values or user CSS any more. `design/` (repo root) holds TOML files: the base tokens and one file per
built-in theme / mode, all in one schema (`design/schema.md` documents it). They hold every value the look uses: colors,
type (families, sizes, line heights, weights), spacing and metrics (row padding, indent, bullet, checkbox, top bar,
phone toolbar, sidebar), radii. Values vary by **modes**: theme (all themes of `apps/web/src/styles.css`), font size
(small / medium / large), density (compact / cozy / comfortable, `docs/03-features-ui.md` §2.3), font family
choices. A generator (Node script, or Style Dictionary) writes:

- a CSS file of custom properties per mode for the web app, which `styles.css` uses instead of its own `:root` blocks
  (`styles.css` keeps only structure: selectors and rules that reference the properties);
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

## 4. User appearance: the same TOML

A user's appearance is a TOML document in the same schema, holding only what they change (for example
`[color] tag = "#008080"`, `[type] size = 17`). Both apps layer it over the chosen theme and modes and apply it at
once, without a restart. It is stored where the other preferences are (locally for now; synced with the other prefs
once sync exists), and can be exported / imported as a `.toml` file, so a custom theme is a file.

- Both settings screens edit it two ways: per-entry controls generated from the schema (color, length, font, weight,
  choice, with ranges), and the TOML text itself, validated with errors shown at their line. The web one is this
  area's; the native one is area 4's.
- **The web's Custom CSS setting goes away** (`apps/web/src/ui/settings.ts`, `custom_css` in `main.ts`), replaced by
  this. What to do with `custom_css` values already stored (ignore and keep, or drop) is the user's call: report it
  to the orchestration session instead of deciding.
- Tests: the same TOML gives the same render model on both apps; every schema entry changes what it says it changes;
  invalid TOML is rejected with a located error and leaves the look as it was.

## Done when

Tokens and spec are the single source (no color or metric literals left in `styles.css` or `style.rs` outside the
generated files), the web QA still passes, the cross-checks run in CI, the user TOML tested on both apps, the web's Custom CSS removed, TASKS.md updated,
committed and pushed, working tree clean.
