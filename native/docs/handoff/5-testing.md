# Area 5: deterministic, solid tests for the native app

The user wants the native app verified deterministically, against the oracles the repository already has, rather
than by screenshots. Areas 2-4 (thread, sidebar, themes) are on hold; this comes after area 1 (inline syntax).

## Oracles we already have

- The TS core (`packages/core`): differential fixtures, as `outliner-core` already does.
- Dynalist itself, recorded: `qa/golden/editing.json` (398 keyboard scenarios: `pre` / `post` normalized states
  with items, depth, props, caret / selection, zoom; the steps are defined in `qa/suites/editing.mjs`, known
  Dynalist bugs in `qa/lib/compare.mjs` `KNOWN`), `qa/golden/fuzz.json.gz` (random key runs, 400 seeds x 10
  steps), `qa/golden/layout.json` and `layout-mobile.json` (measured type and geometry, compared with tolerances by
  `qa/layout.mjs`). The web app passes all of them; read `qa/PLAN.md` first.

## Work, in this order

1. **Editing behavior against Dynalist.** Move the key handling out of the NSH view into a pure Rust module (an
   "editor" layer in `outliner-app` or a new crate, no NSH types): a key or typed text plus the current
   caret/selection goes to a command on the document and gives the new caret/selection. The NSH row input then
   only forwards keys and text to it. Export the scenarios to JSON with a small Node script (definitions from
   `qa/suites/editing.mjs`, golden states from `qa/golden/editing.json`, the `KNOWN` list), and replay every
   scenario in a Rust test: build `pre` from the fixture and setup, run the steps, normalize, compare with `post`
   as `qa/lib/compare.mjs` does (same tolerances, same KNOWN exceptions). Then the fuzz recording the same way.
   Report the match count; scenarios that need things the native app does not have yet (block selection,
   clipboard, notes editing, search ...) are listed as skipped with the reason, never counted as passing.
2. **Look against Dynalist's measurements.** A headless NSH test (`nsh-testing`: `screen().layout_nodes()`,
   `span_style`) that builds the same fixture as `qa/layout.mjs`, measures the same metrics, and compares them with
   `qa/golden/layout.json` (desktop: `Os::MacOs` + `Idiom::Desktop`) and `layout-mobile.json` (phone, 390x844)
   with the same tolerances. Font family differs by design (Fira Sans vs Whitney): treat it as the web QA does.
   Add a structural snapshot (layout tree, texts, span styles, in a stable text form, checked in) to catch
   unintended changes.
3. **Core invariants.** Property tests (seeded, e.g. `proptest`) on `outliner-core`: no cycles, parent/children
   consistency, serialize round trip, undo then redo restores the canonical tree, any interleaving of several
   clients through a simulated server converges. Run `cargo-mutants` on `outliner-core` once and report the
   caught / missed ratio (do not add it to the PR-blocking CI if it is slow; a scheduled or manual workflow is fine).
4. **Determinism hygiene.** `rust-toolchain.toml` pinning the toolchain, fixed seeds with failing seeds kept as
   regression cases, no sleeps in tests (wait for conditions), no network. All of it in CI (`ci.yml`), and a CI
   step that runs the test suites a few times to catch flaky tests.

## Done when

Each oracle above is replayed by a Rust test that runs in CI; the report (in TASKS.md, Korean) gives the numbers:
scenarios matching / skipped (by reason) / failing, layout metrics within tolerance, mutants caught. Failing
behavior that the app should already have is fixed in the app or core (or, if the fix is large, listed with its
cause). Everything committed and pushed, working tree clean.
