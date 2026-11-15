# Area 7: the rest of the core ports

Port the remaining modules of `packages/core` to `crates/outliner-core`, each checked against the TS code with a
fixture generator and a replay test, as `commands`, `inline` and `dates` already are (see common.md, "How we
verify"; check that the tests catch mutations, in a scratch copy with its own target dir):

- `search.ts` (query language: `compileQuery`, `searchTree`, filters such as `is:completed`, tags, dates, the
  highlight ranges `searchTree` reports, `fuzzyFilter`),
- `refs.ts` (`[[` item links, `parseItemUrl`, backlinks: `findBacklinks`, `backlinkCounts`),
- `paste.ts` (plain text / indented text / HTML / OPML import into `Portable` items, `toPortable` copies),
- `export.ts` (plain text, Markdown, OPML, HTML export, the HTML renderer of `inline.ts` it needs),
- `calendar.ts` (ICS generation).

Only `crates/outliner-core` (and its fixtures and tests) changes: the app uses these later (area 9 / 10). Run in
parallel with other sessions: commit small, `git pull --rebase` before each push.

## Done when

Every module ported with a passing replay test that catches mutations, CI `native-core` regenerates the new
fixtures, cargo test (twice) and clippy pass, TASKS.md updated, committed and pushed, working tree clean.
