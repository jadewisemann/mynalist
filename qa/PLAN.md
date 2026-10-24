# Dynalist parity QA

Goal: measure, and drive up, how closely this outliner behaves and looks like Dynalist. Dynalist itself is
the oracle. Every scenario runs on the live public demo (`https://dynalist.io/demo`, no account) and on our
build, side by side in headless Chromium, and the two results are compared. It is a black-box comparison:
no Dynalist code, fonts or assets are used, only what the demo does in a browser.

```
npm run qa:serve        # build + serve ours on :5174 (vite preview)
npm run qa              # replay: ours vs the recorded Dynalist results; offline, exits 1 on regression
npm run qa:live         # live: both apps side by side (needs dynalist.io)
npm run qa:record       # live, and re-record qa/golden/ from Dynalist

node qa/run.mjs <editing|fuzz|render> [--replay|--record] [--only=undo/,nav/] [--workers=N] [--verbose]
    live-only: [--stability] [--no-confirm]; replay-only: [--accept]
FUZZ_N=200 FUZZ_FROM=1001 node qa/run.mjs fuzz     # explore new seeds (the recording covers 1–400 × 10)
node qa/layout.mjs [--replay|--record]
node qa/show.mjs <suite> [id-substring]           # side-by-side diffs in the terminal, with trace context
```

Each run writes `qa/out/<suite>/report.html`: a bar per group, then every scenario with both outlines next
to each other (differing lines highlighted), clipboard contents and screenshots. A "hide matches" toggle
is on by default. `results.json` next to it holds the raw states.

## Live, record, replay

The live demo is the source of truth, but it is slow, needs the network and has its own timing noise.
So its results are **recorded** once into `qa/golden/` and **replayed**: a replay runs only our build and
compares it with the recording, using exactly the same verdict logic (`qa/lib/compare.mjs`). That makes the
parity suites an ordinary regression test. CI runs it (`.github/workflows/ci.yml`) and uploads the HTML
reports as an artifact.

- **Determinism:** every scenario runs at a fixed instant (`QA_NOW` in `qa/lib/runner.mjs`, clock still
  flowing) in UTC. Date labels such as "Tomorrow" or "in 3 hours" therefore come out the same on any day,
  on any machine.
- **What is stored:** per scenario, Dynalist's normalized states: before, after, and after every step for
  fuzz, plus the clipboard. Also a fingerprint of the scenario definition, and whether our build matched at
  recording time. Flaky scenarios store the result of the lone re-run. Screenshots are not stored. Neither is Dynalist's own demo content: a scenario where undo
  brings it back (`demo-reset-in-undo`) keeps only its verdict and is skipped in replays.
- **Ratchet:** a replay fails on a **regression** (a scenario that matched when recorded no longer does), or
  on an **unrecorded** scenario (new, or its steps changed, so its fingerprint differs). A scenario that never
  matched is reported as a **known diff** and does not fail. One that starts matching is reported as **fixed**.
  `--accept` writes the current verdicts into the baseline.
- **When to re-record:** after adding or changing scenarios (`--record --only=…` merges just those), or
  to pick up changes in Dynalist itself. A live run against the old recording shows what moved.

To add a scenario: append an `add(group, id, fixture, steps, setup?, extra?)` line to `qa/suites/editing.mjs`
(steps: key names, `{type}`, `{paste}`, `{at:[item, offset, field]}`, `{copy}`, `{cut}`, `{wait}`). Then run
`node qa/run.mjs editing --record --only=<id>`. To compare against another app, write an adapter with
the same five functions as the two in `qa/lib/apps.mjs`.

## Layers

| suite | what | how it compares |
|---|---|---|
| `editing` | ~340 hand-written scenarios in 17 groups: enter, backspace, delete, delitem, indent, outdent, move, nav, select, props, collapse, zoom, undo, format, typing, paste, copy | normalized editor state after the steps |
| `fuzz` | seeded random key sequences (28 keys incl. typing, undo/redo, block selection, moves) over 5 fixtures | state after **every** step; reports the first divergent step |
| `render` | ~120 inputs for the inline syntax: emphasis, empty pairs, links, tags, dates, recurrence, relative dates, LaTeX, code | styled text runs of the rendered (not edited) item |
| `layout` | one rich fixture: computed font size/weight/line height/colour, row pitch, indent, bullet/checkbox geometry, inline element styles | 129 metrics with tolerances |
| `apps/web/e2e/*` | our own end-to-end checks (keys, popups, drag & drop, mobile toolbar, two-client sync) | expectations; sync has no Dynalist counterpart in the demo |
| `packages/core/test/parity.test.ts` | the behaviour rules found here, pinned at the core level | unit tests |

### Normalized state

```
{ zoom, title, items: [{ d: depth, c: text, n: note, ck: checked, cb: checkbox, h: heading, num, col: collapsed }],
  focus: { i: item index (-1 = title), f: 'c'|'n', s, e } | null, selected: [item indexes] }
```

On Dynalist it is read from the DOM: `.Node.is-currentRoot` and the `.Node-content`/`.Node-note` text, the
`is-checked`/`has-checkbox`/`is-headingN`/`is-numbered`/`is-collapsed`/`is-selected` classes, `#z=` in the URL
for zoom, and the window selection for the caret. On ours it comes from `window.__qa()` (`apps/web/src/qa.ts`).
Both are printed as the same outline text (`•`, `▸` collapsed, `[x]`, `H1`, `|` caret, `[…]` selection, `▌` block
selection, `¶` note), so a diff can be read directly.

### Driving both apps the same way

- **Fixtures** go in through a synthetic `paste` event carrying `text/plain`. Typing them would trigger
  Dynalist's `#`/`@` autocomplete. Setup operations (note, check, collapse, checkbox, heading, numbered) are
  applied with the apps' own shortcuts, from the last item up.
- **Fresh document:** Dynalist's demo is reset with select-all + delete. This is retried until it is empty,
  then the title is set to "Welcome to Dynalist" so both apps start alike.
- **Key timing:** 60 ms between keys. Faster, and Dynalist reorders some keys.
- **Isolation:** every scenario runs in a fresh browser context, and each app gets its own.

## Verdicts

| status | meaning |
|---|---|
| `match` | identical state (or within tolerance, below) |
| `diff` | a divergence that reproduces when the scenario runs alone |
| `flaky` | diverged under parallel load but matched when re-run alone. This is Dynalist timing, mostly undo grouping. Every `diff` gets a serial confirm pass; `--stability` also runs Dynalist twice. |
| `geometry` | fuzz only: the first divergence is an ↑/↓ landing exactly one character off (font metrics; later steps inherit it) |
| `known` | fuzz only: matches a documented Dynalist quirk (see below) |
| `setup` | the fixture itself came out different (harness problem, not a verdict) |
| `error` | Dynalist failed to load or run (network) |

### Tolerances

- **Caret ±1 character** on vertical moves between rows at different x offsets, and between title and items.
  Dynalist maps the caret by pixel x, and our font (Fira Sans) is ~4% wider than Dynalist's Whitney, so the
  same pixel can fall on a neighbouring character. These scenarios are marked `GEOM` in the suite.
  Title ↔ item moves get ±2 (`GEOM_TITLE`): the title is larger and offset, and different Chromium builds
  (e.g. CI) shift that mapping by one more character.
- **Layout:** numbers within max(1.5px, 8%), colours within RGB distance 40, font family not compared.
- **Render:** Dynalist's highlight.js spans inside code blocks are ignored, so only the text is compared.

## Known Dynalist quirks (classified, not copied)

- **Tab with nothing focused** is not handled by Dynalist. The browser moves focus to Dynalist's
  focus-receiver element, which block-selects an item. Ours swallows bound keys.
- **"Untitled" placeholder:** typing into an empty title, or merging an item into it, keeps the placeholder
  as real text ("Untitledab").
- **Enter at the very start of the title** moves it into a new first item, and Ctrl+Z does not revert that.
- **End then Enter while the title has its hidden selection** (after Shift+↑/↓) inserts a raw newline into
  the title.
- **Demo reset in undo history:** the harness resets the demo by deleting its content. In fuzz, enough
  Ctrl+Z presses bring that content back. This is a harness artifact.
- **Exceptions:** Dynalist occasionally throws (`generate_loc_id` of null) mid-scenario.

## Untestable on the demo

- **HTML paste / rich round-trip:** Dynalist ignores synthetic paste events that carry `text/html`. Only
  plain-text paste is compared. Copy is compared in both flavours: `text/plain` is flat lines; the HTML is
  `<ul><li>…</li></ul>`.
- **Sync, multiple documents, search across documents, settings:** the demo is a single local document.
  These are covered by our e2e (`sync.mjs`) and unit tests.
- **Colour labels (Ctrl+Shift+L)** didn't apply in the demo.

## Known gaps on our side

- Code blocks: no syntax highlighting and no Copy button.

## Rules learned from Dynalist (all implemented)

The behaviours below were found by this harness, mostly by fuzz. Each now matches Dynalist.

- **Caret, vertical:**
  - ↑/↓ skip notes and keep no sticky goal column (pixel x each time).
  - From the start of a line you land on the start of the target line.
  - At the first or last item the caret stays put.
- **Caret, other:**
  - Zooming in or out keeps the caret offset.
  - Checking an item moves the caret to the start of the next item that is not a descendant.
- **Escape:**
  - Escape toggles editing; a second Escape resumes where you were. After a block selection it resumes at
    the start of the first selected item.
  - Escape in the title moves the caret to 0 and keeps editing.
- **Hidden title selection:**
  - Shift+↑/↓ in the title puts the caret at 0 and block-selects the title invisibly.
  - Until Escape, ↑ or ←: → jumps to the end of the last item, and Backspace/Delete/Enter/↓ do nothing.
- **Backspace and Delete:**
  - Backspace at the start of the first item merges it into the title, unless it has children or a note.
  - Delete at the end merges the next item in, and the merged item takes the next item's place.
  - An empty item just disappears.
  - Nothing happens when the next item is an only child with children of its own.
- **Selected items:**
  - Deleting them with Backspace puts the caret on the previous item (never the title); with Delete, on the
    next one.
  - → goes to the end of the last visible descendant.
  - ↓ with nothing below lands on the last selected item.
- **Moving items:** a first/last child moves into the parent's previous/next sibling, even a childless one,
  unless that sibling is collapsed.
- **Undo caret:**
  - After a pure text undo of typing, the caret goes back to where the typing started.
  - After undoing deletions, it goes where it was right after the first deletion of the undo group. This
    applies to Backspace, Delete and Ctrl+Backspace alike.
- **Keys:** bound keys never fall through to the browser, even when they do nothing (e.g. Tab on a first
  child).
- **Paste and copy:**
  - Plain-text paste keeps "- " and "* " markers and blank lines (as empty items). A single line is
    left-trimmed.
  - Copy writes flat `text/plain`.
- **Rendering:**
  - Empty emphasis or code pairs render as nothing.
  - Link text is literal.
  - Date labels look like "Tue, January 2nd 2024, 10 am" (with ", every week" for recurrence). A timed date
    today shows as relative ("in 14 hours").
- **Layout:** the metrics were measured from Dynalist (sizes, line heights, colours, spacing) and are listed
  in `qa/layout.mjs`'s report.
