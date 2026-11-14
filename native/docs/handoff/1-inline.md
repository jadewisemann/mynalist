# Area 1: inline syntax, notes, numbered lists

## Goal

Rows render exactly as the web app renders them: every inline construct of `packages/core/src/inline.ts` (bold
`**`, italic `__`, strike `~~`, highlight `==`, code, code blocks, links, images, tags, `[[` item links, dates, LaTeX,
...), plus item notes (prop `n`, the small gray line under the content) and numbered children (prop `num` on the
parent: numbers instead of bullets).

## Work

1. Port `inline.ts` to `crates/outliner-core/src/inline.rs` (and what it needs from `dates.ts`; port `dates.ts` too
   if the inline parser depends on it). Add `fixtures/gen-inline.ts` + `tests/inline.rs` checking the parse output
   (and helpers such as `inlineToPlain`, `extractTags`, `extractDates`) against TS on many inputs: the web's own tests
   in `packages/core/test/inline.test.ts` and `dates.test.ts` are a good corpus, plus random markup.
2. Then wire the date-dependent parts left out of `commands.rs`: recurring dates on check
   (`CheckOptions.recurring`, `advanceRecurrence`) and the date sort modes (`DateFn`); extend `gen-commands.ts`.
3. Replace `crates/outliner-app/src/inline.rs` with the core parser and render every node type as the web does
   (styles in `apps/web/src/styles.css`, `.o-*` classes; the renderer is `packages/core/src/inline.ts` render +
   `apps/web/src/editor/`). Date chips, highlight, strike, images (as NSH `image`), code blocks, KaTeX (if NSH has no
   math rendering, show the source in mono and say so).
4. Notes: draw `n` under the content (`.o-note`: 0.8333em, color `--note`, line height 1.2). Editing a note can wait
   (say so in TASKS.md) unless it is quick.
5. Numbered lists: a parent with `num` shows its children's numbers in the bullet column (`.o-numbered`: 28px wide,
   right aligned, tabular figures).

## Done when

Core inline/dates parity tests pass (and catch mutations), the app renders the web's inline kinds, notes and
numbered lists, headless tests cover them, screenshots match the web app for a document using them, CI is green,
TASKS.md updated, everything committed and pushed.
